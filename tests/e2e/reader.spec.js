const { test, expect, baseUrl, appOrigin, tinyPng, siteManifest, fileEntry, dirEntry, bundleEntry, manifestDocument } = require('./support/fixtures');
const { makeLedger, makeLedgerEntry, normalizedSha } = require('./support/ledger');

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
  const manifest = manifestDocument([
    ...siteManifest.entries,
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
  ]);

  responses.set('/content/manifest.json', JSON.stringify(manifest));
  responses.set('/content/writing/foo/en.md', '# English Foo\n\nEnglish body.');
  responses.set('/content/writing/foo/ko.md', '# 한국어 Foo\n\n한국어 본문.');
  responses.set('/content/writing/foo/print.pdf', Buffer.from('%PDF-1.4\n%%EOF\n'));
  responses.set('/content/writing/foo/notes/readme.md', '# Notes\n\nNotes body.');
  responses.set('/content/writing/foo/cover.png', tinyPng);
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
  const ledger = makeLedger([
    makeLedgerEntry({
      route: '/writing/foo',
      path: 'writing/foo',
      date: '2026-05-15',
      files: [
        {
          path: 'content/writing/foo/_index.dir.json',
          sha256: normalizedSha('a'),
          bytes: 300
        },
        {
          path: 'content/writing/foo/cover.png',
          sha256: normalizedSha('d'),
          bytes: tinyPng.length
        },
        {
          path: 'content/writing/foo/en.md',
          sha256: normalizedSha('b'),
          bytes: 28
        },
        {
          path: 'content/writing/foo/ko.md',
          sha256: normalizedSha('c'),
          bytes: 24
        },
        {
          path: 'content/writing/foo/print.pdf',
          sha256: normalizedSha('e'),
          bytes: 14
        },
        {
          path: 'content/writing/foo/notes/readme.md',
          sha256: normalizedSha('f'),
          bytes: 14
        }
      ]
    })
  ]);
  responses.set('/content/.websh/ledger.json', JSON.stringify(ledger));
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
    /content\/writing\/foo\/print\.pdf#view=FitH&zoom=page-width$/,
    { timeout: 10000 }
  );
  await expect(page.locator('[aria-current="true"]')).toContainText('Print PDF');

  await page.getByRole('link', { name: 'Notes' }).click();
  await page.waitForURL('**/#/writing/foo/notes');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('readme');
  await expect(page.locator('[aria-current="true"]')).toContainText('Notes');

  await page.goto(`${baseUrl}/#/writing/foo/cover.png`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('img', { name: 'Cover' })).toHaveAttribute('src', /cover\.png/);
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
  const manifest = manifestDocument([
    ...siteManifest.entries,
    fileEntry('docs/math.md', 'Math')
  ]);
  responses.set('/content/manifest.json', JSON.stringify(manifest));
  responses.set('/content/docs/math.md', '# Math\n\nInline $E = mc^2$.\n');

  const katexRequests = [];
  let mathRequests = 0;
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname === '/content/docs/math.md') mathRequests += 1;
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

test('unsigned content hides the signature chip and homepage exposes pending status', async ({ page }) => {
  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body]')).toContainText('old');
  // The verification artifact is deliberately unsigned. Never borrow the owner's
  // release signatures or depend on whether a signing key happens to be installed.
  await expect(page.getByRole('button', { name: 'Signature of this page' })).toHaveCount(0);
  await page.goto(`${baseUrl}/#/`, { waitUntil: 'networkidle' });
  const sigchip = page.getByRole('button', { name: 'Signature of this page' });
  await expect(sigchip).toBeVisible();
  await sigchip.click();
  await expect(page.locator('body')).toContainText('pending signatures');
});
