const { test, expect, baseUrl, appOrigin, rawOrigin, tinyPng, siteEntries, rootPath, fileEntry, dirEntry, bundleEntry, publishRoot, installContentPage } = require('./support/fixtures');

const langStorageKey = 'user.LANG';
const readerTextScaleStorageKey = 'websh.reader.scale';

function installBundleArticleFixture(responses) {
  const bundle = {
    default_variant: { strategy: 'locale', fallback: 'en' },
    variants: [
      { id: 'en', path: 'en.md', label: 'English', locale: 'en' },
      { id: 'ko', path: 'ko.md', label: '한국어', locale: 'ko' },
      { id: 'print_pdf', path: 'print.pdf', label: 'Print PDF' },
      { id: 'notes', path: 'notes', label: 'Notes' }
    ]
  };
  const entries = [
    ...siteEntries,
    dirEntry('writing', 'writing'),
    bundleEntry('writing/foo', 'Foo Bundle', {
      date: '2026-05-15',
      tags: ['zk'],
      description: 'One work with two language variants.',
      bundle
    }),
    fileEntry('writing/foo/en.md', 'English Foo', {
      date: '2026-05-15',
      tags: ['zk'],
      description: 'English rendition.'
    }),
    fileEntry('writing/foo/ko.md', '한국어 Foo', {
      date: '2026-05-15',
      tags: ['zk'],
      description: '한국어 rendition.'
    }),
    fileEntry('writing/foo/print.pdf', 'Print PDF', {
      kind: 'document',
      date: '2026-05-15',
      tags: ['zk']
    }),
    dirEntry('writing/foo/notes', 'Notes', {
      date: '2026-05-15',
      tags: ['zk'],
      childCount: 1
    }),
    fileEntry('writing/foo/notes/readme.md', 'Notes Readme', {
      date: '2026-05-15',
      tags: ['zk']
    }),
    fileEntry('writing/foo/cover.png', 'Cover', { kind: 'asset' })
  ];

  publishRoot(responses,{entries,publications:['writing/foo'],files:{
    'writing/foo/en.md':'# English Foo\n\nEnglish body.',
    'writing/foo/ko.md':'# 한국어 Foo\n\n한국어 본문.',
    'writing/foo/print.pdf':Buffer.from('%PDF-1.4\n%%EOF\n'),
    'writing/foo/notes/readme.md':'# Notes\n\nNotes body.',
    'writing/foo/cover.png':tinyPng,
  }});
}


test('reader actions menu controls text size and copies current link', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: appOrigin });

  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('old', { timeout: 10000 });
  await expect(page.locator('[data-reader-body="true"]')).toHaveAttribute('data-text-scale', 'normal');

  await page.getByRole('button', { name: 'Reader actions' }).click();
  await expect(page.getByRole('dialog', { name: 'Reader actions' })).toBeVisible();
  await page.getByRole('button', { name: 'Increase text size' }).click();
  await expect(page.locator('[data-reader-body="true"]')).toHaveAttribute('data-text-scale', 'large');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), readerTextScaleStorageKey)).toBe('large');

  await page.getByRole('button', { name: 'copy link' }).click();
  await expect(page.getByRole('button', { name: /copied/i })).toBeVisible();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(`${baseUrl}/#/docs/old`);

  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Reader actions' })).toBeHidden();

  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body="true"]')).toHaveAttribute('data-text-scale', 'large');
});

test('bundle locale article routes select variants without duplicate home entries', async ({ page, responses }) => {
  installBundleArticleFixture(responses);
  await page.addInitScript((key) => {
    if (!localStorage.getItem(key)) localStorage.setItem(key, 'en');
  }, langStorageKey);

  await page.goto(`${baseUrl}/#/`, { waitUntil: 'networkidle' });
  const writingLink = page
    .getByRole('navigation', { name: 'Site index' })
    .getByRole('link', { name: /writing/ });
  await expect(writingLink).toContainText('1', { timeout: 10000 });
  await expect(page.getByRole('link', { name: 'Foo Bundle' })).toHaveCount(1);

  await page.goto(`${baseUrl}/#/writing/foo`, { waitUntil: 'networkidle' });
  await page.waitForURL('**/#/writing/foo/en');
  await expect(page.locator('body')).toContainText('English body.', { timeout: 10000 });
  await expect(page.locator('body')).toContainText('Variants');
  await expect(page.getByRole('link', { name: '한국어' })).toHaveAttribute('href', /#\/writing\/foo\/ko$/);
  await page.getByRole('link', { name: '한국어' }).click();
  await page.waitForURL('**/#/writing/foo/ko');
  await expect(page.locator('body')).toContainText('한국어 본문.', { timeout: 10000 });
  await expect(page.getByRole('link', { name: 'English' })).toHaveAttribute('href', /#\/writing\/foo\/en$/);
  await expect(page.locator('[aria-current="true"]')).toContainText('한국어');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), langStorageKey)).toBe('ko');

  await page.goto(`${baseUrl}/#/writing/foo`, { waitUntil: 'networkidle' });
  await page.waitForURL('**/#/writing/foo/ko');
  await expect(page.locator('body')).toContainText('한국어 본문.', { timeout: 10000 });

  await page.goto(`${baseUrl}/#/writing/foo/en`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('English body.', { timeout: 10000 });

  await page.goto(`${baseUrl}/#/writing/foo/ko`, { waitUntil: 'networkidle' });
  await page.getByRole('link', { name: 'Print PDF' }).click();
  await page.waitForURL('**/#/writing/foo/print.pdf');
  await expect(page.locator('iframe')).toHaveAttribute(
    'src',
    /^blob:.*#view=FitH&zoom=page-width$/,
    { timeout: 10000 }
  );
  await expect(page.locator('[aria-current="true"]')).toContainText('Print PDF');

  await page.getByRole('link', { name: 'Notes' }).click();
  await page.waitForURL('**/#/writing/foo/notes');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('readme');
  await expect(page.locator('[aria-current="true"]')).toContainText('Notes');

  await page.goto(`${baseUrl}/#/writing/foo/cover.png`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('img', { name: 'Cover' })).toHaveAttribute('src', /^blob:/);
});

test('browser language initializes LANG and selects locale bundle route', async ({ page, responses }) => {
  installBundleArticleFixture(responses);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'languages', {
      configurable: true,
      get: () => ['ko-KR', 'en-US']
    });
    Object.defineProperty(navigator, 'language', {
      configurable: true,
      get: () => 'ko-KR'
    });
  });

  await page.goto(`${baseUrl}/#/writing/foo`, { waitUntil: 'networkidle' });
  await page.waitForURL('**/#/writing/foo/ko');
  await expect(page.locator('body')).toContainText('한국어 본문.', { timeout: 10000 });
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), langStorageKey)).toBe('ko');
});

test('markdown fetches once and loads KaTeX only when math is rendered', async ({ page, responses }) => {
  installContentPage(responses,'docs/math.md','Math','# Math\n\nInline $E = mc^2$.\n');

  const katexRequests = [];
  let mathRequests = 0;
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname === rootPath('docs/math.md')) mathRequests += 1;
    if (/\/assets\/vendor\/katex\/katex\.min\.(css|js)$/.test(url.pathname)) {
      katexRequests.push(url.pathname);
    }
  });

  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body]')).toContainText('old');
  expect(katexRequests).toEqual([]);
  await page.goto(`${baseUrl}/#/docs/math`, { waitUntil: 'networkidle' });
  await expect(page.locator('.katex')).toHaveCount(1, { timeout: 10000 });
  expect(mathRequests).toBe(1);
  expect(katexRequests.sort()).toEqual([
    '/assets/vendor/katex/katex.min.css',
    '/assets/vendor/katex/katex.min.js'
  ]);

  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await page.goto(`${baseUrl}/#/docs/math`, { waitUntil: 'networkidle' });
  await expect(page.locator('.katex')).toHaveCount(1, { timeout: 10000 });
  expect(mathRequests).toBe(1);
  expect(katexRequests.sort()).toEqual([
    '/assets/vendor/katex/katex.min.css',
    '/assets/vendor/katex/katex.min.js'
  ]);
});

test('relative images resolve through authenticated bytes before rendering', async ({page,responses})=>{
 publishRoot(responses,{entries:[...siteEntries,fileEntry('docs/figure.md','Figure'),fileEntry('docs/pixel.png','Pixel',{kind:'asset'})],files:{'docs/figure.md':'# Figure\n\n![Local figure](pixel.png)','docs/pixel.png':tinyPng}});
 await page.goto(`${baseUrl}/#/docs/figure`,{waitUntil:'networkidle'});
 await expect(page.getByRole('img',{name:'Local figure'})).toHaveAttribute('src',/^blob:/);
 await expect(page.getByRole('img',{name:'Local figure'})).toBeVisible();
});

test('a file that differs from its authenticated hash is rejected',async ({page,responses})=>{
 responses.set(rootPath('docs/old.md'),'bad');
 await page.goto(`${baseUrl}/#/docs/old`,{waitUntil:'networkidle'});
 await expect(page.locator('[data-reader-body]')).toContainText('integrity mismatch');
 await expect(page.locator('[data-reader-body]')).not.toHaveText('bad');
});

test('root documents cannot embed bytes from an independently trusted mount',async ({page,responses})=>{
 installContentPage(responses,'docs/cross.md','Cross-source image','# Cross-source image\n\n![Unsigned image](/db/pixel.png)');
 const images=[];page.on('request',request=>{if(request.url().includes('pixel.png'))images.push(request.url());});
 await page.goto(`${baseUrl}/#/docs/cross`,{waitUntil:'networkidle'});
 await expect(page.locator('[data-reader-body]')).toContainText('image escapes its content source');
 await expect(page.getByRole('img',{name:'Unsigned image'})).toHaveCount(0);
 expect(images).toEqual([]);
});
