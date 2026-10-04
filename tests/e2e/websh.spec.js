const { test, expect } = require('playwright/test');
const crypto = require('crypto');

const baseUrl = process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173';
const appBaseUrl = baseUrl.replace(/\/+$/, '');
const appOrigin = new URL(appBaseUrl).origin;
const walletAddress = '0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a';
const themeStorageKey = 'user.THEME';
const langStorageKey = 'user.LANG';
const readerTextScaleStorageKey = 'websh.reader.scale';
const tinyPng = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=',
  'base64'
);

function nodeMetadata(kind, { title, description = null, date = null, tags = [], size = null, modified = null, access = null, renderer = null, childCount = null, bundle = null } = {}) {
  const authored = {};
  if (title !== undefined && title !== null) authored.title = title;
  if (description !== null) authored.description = description;
  if (date !== null) authored.date = date;
  if (tags.length > 0) authored.tags = tags;
  if (access !== null) authored.access = access;

  const derived = {};
  if (renderer !== null) derived.renderer = renderer;
  if (size !== null) derived.size_bytes = size;
  if (modified !== null) derived.modified_at = modified;
  if (childCount !== null) derived.child_count = childCount;

  const metadata = {
    kind,
    authored,
    derived
  };
  if (bundle !== null) metadata.bundle = bundle;
  return metadata;
}

function fileEntry(path, title, options = {}) {
  const ext = path.split('.').pop();
  const kind = options.kind || (ext === 'md' || ext === 'html' ? 'page' : ext === 'pdf' ? 'document' : 'data');
  return {
    path,
    metadata: nodeMetadata(kind, {
      title,
      renderer: kind === 'page' && ext === 'html' ? 'html_page' : kind === 'page' && ext === 'md' ? 'markdown_page' : kind === 'document' && ext === 'pdf' ? 'pdf' : null,
      ...options
    })
  };
}

function dirEntry(path, title, options = {}) {
  return {
    path,
    metadata: nodeMetadata('directory', { title, renderer: 'directory_listing', ...options })
  };
}

function bundleEntry(path, title, options = {}) {
  return {
    path,
    metadata: nodeMetadata('bundle', { title, ...options })
  };
}

function manifestDocument(entries) {
  return { entries };
}

const siteEntries = [
  dirEntry('', 'Home'),
  dirEntry('.site', 'Site support', {
    description: 'Runtime support and trust metadata for the site.',
    tags: ['runtime', 'trust'],
    childCount: 3
  }),
  dirEntry('.websh', '.websh'),
  dirEntry('.websh/mounts', 'mounts'),
  dirEntry('docs', 'docs'),
  dirEntry('docs/deep', 'deep'),
  fileEntry('.websh/ledger.json', 'Ledger', { kind: 'data' }),
  fileEntry('.websh/mounts/db.mount.json', 'DB mount', { kind: 'data' }),
  fileEntry('docs/deep/old.md', 'Deep Old'),
  fileEntry('docs/old.md', 'Old')
];

const siteManifest = manifestDocument(siteEntries);

const dbManifest = manifestDocument([
  dirEntry('', 'DB'),
  fileEntry('fresh.md', 'Fresh')
]);

let rawResponses;

function fixturePathname(url) {
  return url.pathname.replace(/^\/ipfs\/[^/]+(?=\/)/, '');
}

function sha256Json(value) {
  return `0x${crypto.createHash('sha256').update(JSON.stringify(value)).digest('hex')}`;
}

function normalizedSha(ch) {
  return `0x${ch.repeat(64)}`;
}

const genesisHash = normalizedSha('0');

function categoryForPath(path) {
  const category = path.replace(/^\/+/, '').split('/')[0] || '';
  return ['writing', 'projects', 'papers', 'talks'].includes(category) ? category : 'misc';
}

function makeLedgerEntry({ route, path, files, date = null }) {
  const content_sha256 = sha256Json(files);
  const entry = {
    id: `route:${route}`,
    route,
    path,
    category: categoryForPath(path),
    content_files: files,
    content_sha256
  };
  return {
    sort_key: { date, path },
    entry
  };
}

function makeLedger(inputs) {
  const blocks = [...inputs].sort((left, right) => {
    const leftDate = left.sort_key.date;
    const rightDate = right.sort_key.date;
    if (leftDate === null && rightDate !== null) return -1;
    if (leftDate !== null && rightDate === null) return 1;
    if (leftDate !== rightDate) return leftDate < rightDate ? -1 : 1;
    if (left.sort_key.path !== right.sort_key.path) {
      return left.sort_key.path < right.sort_key.path ? -1 : 1;
    }
    return 0;
  }).map((input, index) => ({
    height: index + 1,
    sort_key: input.sort_key,
    prev_block_sha256: index === 0 ? genesisHash : '',
    block_sha256: '',
    entry: input.entry
  }));

  for (let index = 0; index < blocks.length; index += 1) {
    if (index > 0) {
      blocks[index].prev_block_sha256 = blocks[index - 1].block_sha256;
    }
    const block = blocks[index];
    block.block_sha256 = sha256Json({
      height: block.height,
      sort_key: block.sort_key,
      prev_block_sha256: block.prev_block_sha256,
      entry: block.entry
    });
  }

  return {
    version: 1,
    scheme: 'websh.content-ledger.v1',
    hash: 'sha256',
    genesis_hash: genesisHash,
    blocks,
    block_count: blocks.length,
    chain_head: blocks.length === 0 ? genesisHash : blocks[blocks.length - 1].block_sha256
  };
}

function contentTypeForPath(pathname) {
  if (pathname.endsWith('.json')) return 'application/json';
  if (pathname.endsWith('.pdf')) return 'application/pdf';
  if (pathname.endsWith('.png')) return 'image/png';
  if (pathname.endsWith('.svg')) return 'image/svg+xml';
  return 'text/plain';
}

function deferred() {
  let resolve;
  const promise = new Promise((innerResolve) => {
    resolve = innerResolve;
  });
  return { promise, resolve };
}

function freshRawResponses() {
  return new Map([
    ['/content/manifest.json', JSON.stringify(siteManifest)],
    ['/content/docs/old.md', 'old'],
    ['/content/docs/deep/old.md', 'deep old'],
    ['/content/.websh/ledger.json', JSON.stringify(makeLedger([]))],
    ['/content/.websh/mounts/db.mount.json', JSON.stringify({
      backend: 'github',
      mount_at: '/db',
      repo: '0xwonj/mount-db',
      branch: 'main',
      root: '',
      name: 'db',
    })],
    ['/0xwonj/mount-db/main/manifest.json', JSON.stringify(dbManifest)],
    ['/0xwonj/mount-db/main/fresh.md', '# Fresh']
  ]);
}

function contentPathEntries(path, title) {
  const parts = path.split('/').filter(Boolean);
  const dirs = [];
  for (let idx = 0; idx < parts.length - 1; idx += 1) {
    const dirPath = parts.slice(0, idx + 1).join('/');
    dirs.push(dirEntry(dirPath, parts[idx]));
  }
  return [...dirs, fileEntry(path, title)];
}

function installContentPage(path, title, body = '# Fixture page') {
  rawResponses.set(
    '/content/manifest.json',
    JSON.stringify(manifestDocument([
      ...siteEntries,
      ...contentPathEntries(path, title)
    ]))
  );
  rawResponses.set(`/content/${path}`, body);
}

function installBundleArticleFixture() {
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

  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/writing/foo/en.md', '# English Foo\n\nEnglish body.');
  rawResponses.set('/content/writing/foo/ko.md', '# 한국어 Foo\n\n한국어 본문.');
  rawResponses.set('/content/writing/foo/cover.png', tinyPng);
}

async function readBreadcrumbLayout(page) {
  return page.evaluate(() => {
    const breadcrumb = document.querySelector('[data-chrome-role="breadcrumb"]');
    const nav = document.querySelector('[data-chrome-role="nav"]');
    const current = document.querySelector('[data-breadcrumb-current="true"]');
    if (!breadcrumb || !nav || !current) return null;

    const breadcrumbRect = breadcrumb.getBoundingClientRect();
    const navRect = nav.getBoundingClientRect();
    const overlapsVertically = breadcrumbRect.top < navRect.bottom && breadcrumbRect.bottom > navRect.top;
    const overlapsHorizontally = breadcrumbRect.left < navRect.right && breadcrumbRect.right > navRect.left;
    const chromeBlockerOverlaps = Array.from(document.querySelectorAll('[data-chrome-role]'))
      .filter((element) => element !== breadcrumb)
      .map((element) => {
        const rect = element.getBoundingClientRect();
        const vertical = breadcrumbRect.top < rect.bottom && breadcrumbRect.bottom > rect.top;
        const horizontal = breadcrumbRect.left < rect.right && breadcrumbRect.right > rect.left;
        return {
          role: element.getAttribute('data-chrome-role'),
          overlaps: vertical && horizontal
        };
      })
      .filter((entry) => entry.overlaps)
      .map((entry) => entry.role);
    const crumbChromeBlockerOverlaps = Array.from(breadcrumb.children)
      .flatMap((crumb) => {
        const crumbRect = crumb.getBoundingClientRect();
        return Array.from(document.querySelectorAll('[data-chrome-role]'))
          .filter((element) => element !== breadcrumb)
          .map((element) => {
            const rect = element.getBoundingClientRect();
            const vertical = crumbRect.top < rect.bottom && crumbRect.bottom > rect.top;
            const horizontal = crumbRect.left < rect.right && crumbRect.right > rect.left;
            return {
              crumb: crumb.textContent.trim(),
              role: element.getAttribute('data-chrome-role'),
              overlaps: vertical && horizontal
            };
          })
          .filter((entry) => entry.overlaps)
          .map(({ crumb, role }) => `${crumb}:${role}`);
      });

    return {
      visibleCrumbs: Array.from(
        breadcrumb.querySelectorAll(':scope > a, :scope > span[data-breadcrumb-current]')
      ).map((element) => ({
        type: element.hasAttribute('data-breadcrumb-current') ? 'current' : 'crumb',
        text: element.textContent.trim(),
        width: element.getBoundingClientRect().width,
        clientWidth: element.clientWidth,
        scrollWidth: element.scrollWidth
      })),
      navBreadcrumbOverlap: overlapsVertically && overlapsHorizontally,
      chromeBlockerOverlaps,
      crumbChromeBlockerOverlaps,
      scrollWidth: document.documentElement.scrollWidth,
      viewportWidth: window.innerWidth,
      currentClientWidth: current.clientWidth,
      currentScrollWidth: current.scrollWidth
    };
  });
}

test.beforeEach(async ({ page }) => {
  rawResponses = freshRawResponses();

  await page.addInitScript((connectedAddress) => {
    window.ethereum = {
      request: async ({ method }) => {
        if (method === 'eth_requestAccounts' || method === 'eth_accounts') {
          return [connectedAddress];
        }
        if (method === 'eth_chainId') {
          return '0x1';
        }
        return null;
      }
    };
  }, walletAddress);

  await page.route('https://api.ensideas.com/**', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
  });

  await page.route('**/content/**', async (route) => {
    const url = new URL(route.request().url());
    const body = rawResponses.get(fixturePathname(url));
    if (body === undefined) {
      await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
      return;
    }
    await route.fulfill({ status: 200, contentType: contentTypeForPath(url.pathname), body });
  });

  await page.route('https://raw.githubusercontent.com/**', async (route) => {
    const url = new URL(route.request().url());
    const body = rawResponses.get(fixturePathname(url));
    if (body === undefined) {
      await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
      return;
    }
    await route.fulfill({ status: 200, contentType: contentTypeForPath(url.pathname), body });
  });
});

async function collectBrowserErrors(page) {
  const pageErrors = [];
  const consoleErrors = [];
  page.on('pageerror', (error) => { const details = `${page.url()}: ${error.stack || error.message}`; pageErrors.push(details); console.error(details); });
  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text());
    }
  });
  return { pageErrors, consoleErrors };
}

function collectNavigationNetwork(page) {
  const mainDocumentRequests = [];
  const wasmResponses = [];
  const sameOriginFailures = [];

  page.on('request', (request) => {
    if (request.resourceType() === 'document' && request.frame() === page.mainFrame()) {
      mainDocumentRequests.push(request.url());
    }
  });

  page.on('response', (response) => {
    const url = new URL(response.url());
    if (url.origin === appOrigin && url.pathname.endsWith('_bg.wasm')) {
      wasmResponses.push(response.url());
    }
    if (
      url.origin === appOrigin &&
      response.status() >= 400 &&
      !url.pathname.startsWith('/.well-known/trunk/')
    ) {
      sameOriginFailures.push(`${response.status()} ${url.pathname}`);
    }
  });

  return { mainDocumentRequests, wasmResponses, sameOriginFailures };
}

async function installIpfsBaseAlias(page, cid = 'fakecid') {
  await page.route(`**/ipfs/${cid}/**`, async (route) => {
    const url = new URL(route.request().url());
    const targetPath = `/${url.pathname.replace(new RegExp(`^/ipfs/${cid}/?`), '')}`;
    const response = await route.fetch({ url: `${appOrigin}${targetPath}${url.search}` });
    await route.fulfill({ response });
  });
}

async function runCommand(page, input, expectedText) {
  const body = page.locator('body');
  const before = (await body.textContent()) || '';
  await page.locator('input[type="text"]').fill(input);
  await page.keyboard.press('Enter');
  if (expectedText) {
    expect(before).not.toContain(expectedText);
    await expect(body).toContainText(expectedText, { timeout: 10000 });
  }
}

async function cacheRecords(page) {
  return page.evaluate(async () => {
    if (!(await indexedDB.databases()).some((db) => db.name === 'websh-cache')) return [];
    return new Promise((resolve, reject) => {
      const request = indexedDB.open('websh-cache');
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains('mount_snapshots')) { db.close(); resolve([]); return; }
        const tx = db.transaction('mount_snapshots', 'readonly');
        const read = tx.objectStore('mount_snapshots').getAll();
        tx.oncomplete = () => { db.close(); resolve(read.result); };
        tx.onerror = () => { db.close(); reject(tx.error); };
      };
    });
  });
}

const directLoadCases = [
  ['/#/', 'A Homepage, Formalised'],
  ['/#/websh', 'guest@wonjae.eth:~'],
  ['/#/websh/db', '~/websh/db'],
  ['/#/db/fresh', 'Fresh']
];

test('official root loads built-in homepage', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  const network = collectNavigationNetwork(page);
  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  expect(new URL(page.url()).hash).toBe('#/');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~');
  await expect(page.locator('body')).not.toContainText('No route matched');
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
  expect(network.sameOriginFailures).toEqual([]);
});

test('pwa manifest and touch icon assets are available', async ({ request }) => {
  for (const path of ['/assets/manifest.json', '/assets/favicon.svg']) {
    const response = await request.get(`${appBaseUrl}${path}`);
    expect(response.status(), path).toBe(200);
  }
});

test('official root does not require an index file in the mounted filesystem', async ({ page }) => {
  rawResponses = new Map([
    ['/content/manifest.json', JSON.stringify(manifestDocument([dirEntry('', 'Home')]))]
  ]);

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  expect(new URL(page.url()).hash).toBe('#/');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~');
  await expect(page.locator('body')).not.toContainText('No route matched');
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('home renders static sections while the root manifest is still loading', async ({ page }) => {
  const manifestRequested = deferred();
  const releaseManifest = deferred();
  const manifest = manifestDocument([
    ...siteManifest.entries,
    fileEntry('.site/now.toml', 'Now', { kind: 'document' }),
    dirEntry('writing', 'writing'),
    dirEntry('projects', 'projects'),
    fileEntry('writing/loaded.md', 'Loaded Writing', {
      date: '2026-05-01',
      tags: ['notes']
    }),
    fileEntry('projects/loaded.md', 'Loaded Project', {
      date: '2026-05-02',
      tags: ['rust']
    })
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/.site/now.toml', '[[items]]\ndate = "2026-05-01"\ntext = "Loaded now item"\n');

  await page.route('**/content/manifest.json', async (route) => {
    manifestRequested.resolve();
    await releaseManifest.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: rawResponses.get('/content/manifest.json')
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/`, { waitUntil: 'domcontentloaded' });

  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  const toc = page.getByRole('navigation', { name: 'Site index' });
  const writingLink = toc.getByRole('link', { name: /writing/ });
  await expect(writingLink).toContainText('…');
  await expect(page.locator('body')).not.toContainText('Loaded Project');
  await expect(page.locator('body')).not.toContainText('Loaded now item');

  await manifestRequested.promise;
  releaseManifest.resolve();

  await expect(writingLink).toContainText('1', { timeout: 10000 });
  await expect(page.locator('body')).toContainText('Loaded Project', { timeout: 10000 });
  await expect(page.locator('body')).toContainText('Loaded now item', { timeout: 10000 });
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('home stays quiet when the root manifest fails', async ({ page }) => {
  let nowRequests = 0;
  let ledgerRequests = 0;
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname === '/content/.site/now.toml') {
      nowRequests += 1;
    }
    if (url.pathname === '/content/.websh/ledger.json') {
      ledgerRequests += 1;
    }
  });

  rawResponses.set('/content/.site/now.toml', '[[items]]\ndate = "2026-05-01"\ntext = "should not load"\n');
  rawResponses.set('/content/.websh/ledger.json', JSON.stringify(makeLedger([])));

  await page.route('**/content/manifest.json', async (route) => {
    await route.fulfill({
      status: 404,
      contentType: 'text/plain',
      body: 'root manifest unavailable'
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/`, { waitUntil: 'domcontentloaded' });

  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  const writingLink = page
    .getByRole('navigation', { name: 'Site index' })
    .getByRole('link', { name: /writing/ });
  await expect(writingLink).toContainText('—', { timeout: 10000 });
  expect(nowRequests).toBe(0);
  expect(ledgerRequests).toBe(0);
  expect(pageErrors).toEqual([]);
  expect(consoleErrors.filter((message) => !message.includes('status of 404'))).toEqual([]);
});

test('direct ledger waits for root manifest before reading the content ledger', async ({ page }) => {
  const manifestRequested = deferred();
  const releaseManifest = deferred();
  let ledgerRequests = 0;

  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname === '/content/.websh/ledger.json') {
      ledgerRequests += 1;
    }
  });

  await page.route('**/content/manifest.json', async (route) => {
    manifestRequested.resolve();
    await releaseManifest.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: rawResponses.get('/content/manifest.json')
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'domcontentloaded' });
  await expect(page.locator('body')).toContainText('ledger pending', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~/ledger');

  await manifestRequested.promise;
  expect(ledgerRequests).toBe(0);
  releaseManifest.resolve();

  await expect.poll(() => ledgerRequests).toBe(1);
  await expect(page.locator('body')).toContainText('appendable', { timeout: 10000 });
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('direct ledger with failed root manifest does not read the content ledger', async ({ page }) => {
  let ledgerRequests = 0;
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname === '/content/.websh/ledger.json') {
      ledgerRequests += 1;
    }
  });

  await page.route('**/content/manifest.json', async (route) => {
    await route.fulfill({
      status: 404,
      contentType: 'text/plain',
      body: 'root manifest unavailable'
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'domcontentloaded' });
  await expect(page.locator('body')).toContainText(/root mount failed/i, { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~/ledger');
  expect(ledgerRequests).toBe(0);
  expect(pageErrors).toEqual([]);
  expect(consoleErrors.filter((message) => !message.includes('status of 404'))).toEqual([]);
});

test('direct content hash route stays pending while root manifest loads', async ({ page }) => {
  const manifestRequested = deferred();
  const releaseManifest = deferred();

  await page.route('**/content/manifest.json', async (route) => {
    manifestRequested.resolve();
    await releaseManifest.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: rawResponses.get('/content/manifest.json')
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'domcontentloaded' });
  await manifestRequested.promise;

  await expect(page.locator('body')).toContainText('route pending', { timeout: 10000 });
  await expect(page.locator('body')).not.toContainText('404');
  await expect(page.locator('body')).not.toContainText('No route matched');

  releaseManifest.resolve();
  await expect(page.locator('body')).toContainText('old', { timeout: 10000 });
  await expect(page.locator('body')).not.toContainText('No route matched');
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('missing content hash route shows 404 only after root manifest loads', async ({ page }) => {
  const manifestRequested = deferred();
  const releaseManifest = deferred();

  await page.route('**/content/manifest.json', async (route) => {
    manifestRequested.resolve();
    await releaseManifest.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: rawResponses.get('/content/manifest.json')
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/missing`, { waitUntil: 'domcontentloaded' });
  await manifestRequested.promise;

  await expect(page.locator('body')).toContainText('route pending', { timeout: 10000 });
  await expect(page.locator('body')).not.toContainText('404');
  await expect(page.locator('body')).not.toContainText('No route matched');

  releaseManifest.resolve();
  await expect(page.locator('body')).toContainText('404', { timeout: 10000 });
  await expect(page.locator('body')).toContainText('Page not found');
  await page.getByRole('button', { name: 'Signature of this page' }).click();
  await expect(page.locator('body')).toContainText('/.site');
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('direct content hash route reports root mount failure when manifest fails', async ({ page }) => {
  await page.route('**/content/manifest.json', async (route) => {
    await route.fulfill({
      status: 404,
      contentType: 'text/plain',
      body: 'root manifest unavailable'
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'domcontentloaded' });

  await expect(page.locator('body')).toContainText(/route unconfirmed/i, { timeout: 10000 });
  await expect(page.locator('body')).toContainText('content/manifest.json');
  await expect(page.locator('body')).not.toContainText('404');
  await expect(page.locator('body')).not.toContainText('No route matched');
  expect(pageErrors).toEqual([]);
  expect(consoleErrors.filter((message) => !message.includes('status of 404'))).toEqual([]);
});

test('ledger navigation shares the home prefetch request', async ({ page }) => {
  const ledgerRequested = deferred();
  const releaseLedger = deferred();
  let ledgerRequests = 0;

  await page.route('**/content/.websh/ledger.json', async (route) => {
    ledgerRequests += 1;
    ledgerRequested.resolve();
    await releaseLedger.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: rawResponses.get('/content/.websh/ledger.json')
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/`, { waitUntil: 'domcontentloaded' });
  await ledgerRequested.promise;
  expect(ledgerRequests).toBe(1);

  await page.getByRole('link', { name: 'ledger' }).first().click();
  await page.waitForURL('**/#/ledger');
  await expect(page.locator('body')).toContainText('ledger pending', { timeout: 10000 });
  expect(ledgerRequests).toBe(1);
  releaseLedger.resolve();

  await expect(page.locator('body')).toContainText('appendable', { timeout: 10000 });
  expect(ledgerRequests).toBe(1);
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

for (const [hashPath, expectedText] of directLoadCases) {
  test(`direct load ${hashPath}`, async ({ page }) => {
    const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
    await page.goto(`${baseUrl}${hashPath}`, { waitUntil: 'networkidle' });
    expect(new URL(page.url()).hash).toBe(hashPath.slice(1));
    await expect(page.locator('body')).toContainText(expectedText, { timeout: 10000 });
    await expect(page.locator('body')).not.toContainText('No route matched');
    expect(pageErrors).toEqual([]);
    expect(consoleErrors).toEqual([]);
  });
}

test('site chrome breadcrumb ellipsizes current crumb without collapsing path', async ({ page }) => {
  const path = 'writing/a-current-title-that-is-long-enough-to-need-css-truncation-before-path-collapse.md';
  installContentPage(path, 'Long Current Crumb', '# Long current crumb');
  await page.setViewportSize({ width: 560, height: 720 });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/${path.replace(/\.md$/, '')}`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Long current crumb', { timeout: 10000 });

  await expect(async () => {
    const layout = await readBreadcrumbLayout(page);
    expect(layout).not.toBeNull();
    expect(layout.visibleCrumbs.map((crumb) => crumb.type)).toEqual(['crumb', 'crumb', 'current']);
    expect(layout.navBreadcrumbOverlap).toBe(false);
    expect(layout.chromeBlockerOverlaps).toEqual([]);
    expect(layout.crumbChromeBlockerOverlaps).toEqual([]);
    expect(layout.scrollWidth).toBeLessThanOrEqual(layout.viewportWidth);
    expect(layout.currentScrollWidth).toBeGreaterThan(layout.currentClientWidth);
  }).toPass({ timeout: 10000 });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('site chrome breadcrumb ellipsizes crumbs with the same shrink policy', async ({ page }) => {
  const path = 'writing/zk-proofs-from-a-compiler-perspective/ko.md';
  installContentPage(path, 'ZK Proofs from a Compiler Perspective', '# ZK body');
  await page.setViewportSize({ width: 360, height: 720 });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'networkidle' });

  await expect(async () => {
    const layout = await readBreadcrumbLayout(page);
    expect(layout).not.toBeNull();
    expect(layout.visibleCrumbs.map((crumb) => crumb.type)).toEqual(['crumb', 'current']);
    expect(layout.visibleCrumbs.map((crumb) => crumb.text)).toEqual(['~', 'ledger']);
    expect(layout.navBreadcrumbOverlap).toBe(false);
    expect(layout.chromeBlockerOverlaps).toEqual([]);
    expect(layout.crumbChromeBlockerOverlaps).toEqual([]);
  }).toPass({ timeout: 10000 });

  await page.goto(`${baseUrl}/#/${path.replace(/\.md$/, '')}`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('ZK body', { timeout: 10000 });

  await expect(async () => {
    const layout = await readBreadcrumbLayout(page);
    expect(layout).not.toBeNull();
    expect(layout.visibleCrumbs.map((crumb) => crumb.type)).toEqual(['crumb', 'crumb', 'crumb', 'current']);
    expect(layout.visibleCrumbs.map((crumb) => crumb.text)).toEqual(['~', 'writing', 'zk-proofs-from-a-compiler-perspective', 'ko']);
    const longMiddleCrumb = layout.visibleCrumbs[2];
    const currentCrumb = layout.visibleCrumbs[3];
    expect(longMiddleCrumb.scrollWidth).toBeGreaterThan(longMiddleCrumb.clientWidth);
    expect(currentCrumb.scrollWidth).toBeGreaterThan(currentCrumb.clientWidth);
    expect(layout.navBreadcrumbOverlap).toBe(false);
    expect(layout.chromeBlockerOverlaps).toEqual([]);
    expect(layout.crumbChromeBlockerOverlaps).toEqual([]);
    expect(layout.scrollWidth).toBeLessThanOrEqual(layout.viewportWidth);
  }).toPass({ timeout: 10000 });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('site chrome breadcrumb ellipsizes long paths before colliding on narrow nested routes', async ({ page }) => {
  const path = [
    'writing',
    'research',
    'compiler-notes',
    'zkvm-performance',
    'a-very-long-current-title-that-would-otherwise-collide-with-navigation.md'
  ].join('/');
  installContentPage(path, 'Nested Long Crumb', '# Nested long crumb');

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto(`${baseUrl}/#/${path.replace(/\.md$/, '')}`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Nested long crumb', { timeout: 10000 });

  await expect(async () => {
    const layout = await readBreadcrumbLayout(page);
    expect(layout).not.toBeNull();
    expect(layout.navBreadcrumbOverlap).toBe(false);
    expect(layout.chromeBlockerOverlaps).toEqual([]);
    expect(layout.crumbChromeBlockerOverlaps).toEqual([]);
    expect(layout.scrollWidth).toBeLessThanOrEqual(layout.viewportWidth);
  }).toPass({ timeout: 10000 });

  for (const width of [420, 460, 500, 540, 580, 620, 660, 800, 900]) {
    await page.setViewportSize({ width, height: 720 });
    await expect(async () => {
      const layout = await readBreadcrumbLayout(page);
      expect(layout).not.toBeNull();
      expect(layout.visibleCrumbs.some((crumb) => crumb.scrollWidth > crumb.clientWidth)).toBe(true);
      expect(layout.navBreadcrumbOverlap).toBe(false);
      expect(layout.chromeBlockerOverlaps).toEqual([]);
      expect(layout.crumbChromeBlockerOverlaps).toEqual([]);
      expect(layout.scrollWidth).toBeLessThanOrEqual(layout.viewportWidth);
    }).toPass({ timeout: 10000 });
  }

  await page.setViewportSize({ width: 1280, height: 800 });
  await expect(async () => {
    const layout = await readBreadcrumbLayout(page);
    expect(layout).not.toBeNull();
    expect(layout.navBreadcrumbOverlap).toBe(false);
    expect(layout.chromeBlockerOverlaps).toEqual([]);
    expect(layout.crumbChromeBlockerOverlaps).toEqual([]);
    expect(layout.scrollWidth).toBeLessThanOrEqual(layout.viewportWidth);
  }).toPass({ timeout: 10000 });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('home navigation stays inside the hash router', async ({ page }) => {
  const pageErrors = [];
  const network = collectNavigationNetwork(page);
  page.on('pageerror', (error) => {
    pageErrors.push(`${page.url()}: ${error.stack || error.message}`);
  });

  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('ledger', { timeout: 10000 });

  await page.getByRole('link', { name: 'home' }).first().click();
  await page.waitForURL('**/#/');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });

  await page.getByRole('link', { name: 'ledger' }).first().click();
  await page.waitForURL('**/#/ledger');
  await page.getByRole('link', { name: 'home' }).first().click();
  await page.waitForURL('**/#/');

  expect(network.mainDocumentRequests).toHaveLength(1);
  expect(network.wasmResponses).toHaveLength(1);
  expect(network.sameOriginFailures).toEqual([]);
  expect(pageErrors).toEqual([]);
});

test('ipfs base navigation preserves the hash-router base', async ({ page }) => {
  await installIpfsBaseAlias(page);
  const pageErrors = [];
  const network = collectNavigationNetwork(page);
  page.on('pageerror', (error) => {
    pageErrors.push(`${page.url()}: ${error.stack || error.message}`);
  });

  await page.goto(`${appBaseUrl}/ipfs/fakecid/#/ledger`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('ledger', { timeout: 10000 });
  expect(new URL(page.url()).pathname).toBe('/ipfs/fakecid/');
  expect(new URL(page.url()).hash).toBe('#/ledger');

  await page.getByRole('link', { name: 'home' }).first().click();
  await page.waitForURL('**/ipfs/fakecid/#/');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  expect(new URL(page.url()).pathname).toBe('/ipfs/fakecid/');

  await page.getByRole('link', { name: 'ledger' }).first().click();
  await page.waitForURL('**/ipfs/fakecid/#/ledger');
  expect(new URL(page.url()).pathname).toBe('/ipfs/fakecid/');

  expect(network.mainDocumentRequests).toHaveLength(1);
  expect(network.wasmResponses).toHaveLength(1);
  expect(network.sameOriginFailures).toEqual([]);
  expect(pageErrors).toEqual([]);
});

test('pdf content renders through a direct content iframe', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    fileEntry('docs/sample.pdf', 'Sample PDF', { kind: 'document' })
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/docs/sample.pdf', Buffer.from('%PDF-1.4\n%%EOF\n'));

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/sample.pdf`, { waitUntil: 'networkidle' });
  await expect(page.locator('iframe')).toHaveAttribute(
    'src',
    /content\/docs\/sample\.pdf#view=FitH&zoom=page-width$/,
    { timeout: 10000 }
  );

  expect(pageErrors).toEqual([]);
  expect(consoleErrors.filter((message) => message.includes('Content Security Policy'))).toEqual([]);
});

test('markdown reader fetches source once per route load', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    dirEntry('writing', 'writing'),
    fileEntry('writing/content-backed-homepage.md', 'content-backed homepage')
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/writing/content-backed-homepage.md', '# content-backed homepage');

  let markdownRequests = 0;
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.pathname === '/content/writing/content-backed-homepage.md') {
      markdownRequests += 1;
    }
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/writing/content-backed-homepage`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('content-backed homepage', { timeout: 10000 });

  expect(markdownRequests).toBe(1);
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('reader actions menu controls text size and copies current link', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: appOrigin });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
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
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(`${appBaseUrl}/#/docs/old`);

  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Reader actions' })).toBeHidden();

  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body="true"]')).toHaveAttribute('data-text-scale', 'large');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('bundle locale article routes select variants without duplicate home entries', async ({ page }) => {
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
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/.websh/ledger.json', JSON.stringify(ledger));
  rawResponses.set('/content/writing/foo/en.md', '# English Foo\n\nEnglish body.');
  rawResponses.set('/content/writing/foo/ko.md', '# 한국어 Foo\n\n한국어 본문.');
  rawResponses.set('/content/writing/foo/print.pdf', Buffer.from('%PDF-1.4\n%%EOF\n'));
  rawResponses.set('/content/writing/foo/notes/readme.md', '# Notes\n\nNotes body.');
  rawResponses.set('/content/writing/foo/cover.png', tinyPng);
  await page.addInitScript((key) => {
    if (!localStorage.getItem(key)) localStorage.setItem(key, 'en');
  }, langStorageKey);

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
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

  await page.goto(`${baseUrl}/#/writing/foo/ko.md`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Page not found', { timeout: 10000 });

  await page.goto(`${baseUrl}/#/writing/foo/print_pdf`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Page not found', { timeout: 10000 });

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

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('browser language initializes LANG and selects locale bundle route', async ({ page }) => {
  installBundleArticleFixture();
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

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/writing/foo`, { waitUntil: 'networkidle' });
  await page.waitForURL('**/#/writing/foo/ko');
  await expect(page.locator('body')).toContainText('한국어 본문.', { timeout: 10000 });
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), langStorageKey)).toBe('ko');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('shell LANG selects locale bundle root', async ({ page }) => {
  installBundleArticleFixture();
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);

  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'export LANG=ko');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), langStorageKey)).toBe('ko');

  await page.goto(`${baseUrl}/#/writing/foo`, { waitUntil: 'networkidle' });
  await page.waitForURL('**/#/writing/foo/ko');
  await expect(page.locator('body')).toContainText('한국어 본문.', { timeout: 10000 });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('root content renders before external mount scan resolves', async ({ page }) => {
  let releaseDbManifest;
  const dbManifestGate = new Promise((resolve) => {
    releaseDbManifest = resolve;
  });

  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', async (route) => {
    await dbManifestGate;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(dbManifest)
    });
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'domcontentloaded' });
  await expect(page.locator('body')).toContainText('old', { timeout: 10000 });

  releaseDbManifest();
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Fresh', { timeout: 10000 });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('non-math markdown does not request KaTeX assets', async ({ page }) => {
  const katexRequests = [];
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (/\/assets\/vendor\/katex\/katex\.min\.(css|js)$/.test(url.pathname)) {
      katexRequests.push(url.pathname);
    }
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('old', { timeout: 10000 });

  expect(katexRequests).toEqual([]);
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('math markdown lazy-loads KaTeX once', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    fileEntry('docs/math.md', 'Math')
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/docs/math.md', '# Math\n\nInline $E = mc^2$.\n');

  const katexRequests = [];
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (/\/assets\/vendor\/katex\/katex\.min\.(css|js)$/.test(url.pathname)) {
      katexRequests.push(url.pathname);
    }
  });

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/math`, { waitUntil: 'networkidle' });
  await expect(page.locator('.katex')).toHaveCount(1, { timeout: 10000 });
  expect(katexRequests.sort()).toEqual([
    '/assets/vendor/katex/katex.min.css',
    '/assets/vendor/katex/katex.min.js'
  ]);

  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await page.goto(`${baseUrl}/#/docs/math`, { waitUntil: 'networkidle' });
  await expect(page.locator('.katex')).toHaveCount(1, { timeout: 10000 });
  expect(katexRequests.sort()).toEqual([
    '/assets/vendor/katex/katex.min.css',
    '/assets/vendor/katex/katex.min.js'
  ]);

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('attested renderer page shows the route sigchip', async ({ page }) => {
  const bundle = {
    default_variant: { strategy: 'locale', fallback: 'en' },
    variants: [
      { id: 'en', path: 'en.md', label: 'English', locale: 'en' },
      { id: 'ko', path: 'ko.md', label: '한국어', locale: 'ko' }
    ]
  };
  const manifest = manifestDocument([
    ...siteManifest.entries,
    dirEntry('writing', 'writing'),
    bundleEntry('writing/zk-proofs-from-a-compiler-perspective', 'Zero-Knowledge Proofs, from a Compiler Perspective', {
      date: '2026-05-15',
      tags: ['zk'],
      bundle
    }),
    fileEntry('writing/zk-proofs-from-a-compiler-perspective/en.md', 'Zero-Knowledge Proofs, from a Compiler Perspective'),
    fileEntry('writing/zk-proofs-from-a-compiler-perspective/ko.md', '컴파일러 관점에서 보는 영지식 증명')
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/writing/zk-proofs-from-a-compiler-perspective/en.md', '# Zero-Knowledge Proofs\n\ncontent-backed bundle');
  rawResponses.set('/content/writing/zk-proofs-from-a-compiler-perspective/ko.md', '# 컴파일러 관점에서 보는 영지식 증명');

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/writing/zk-proofs-from-a-compiler-perspective`, { waitUntil: 'networkidle' });
  await page.waitForURL('**/#/writing/zk-proofs-from-a-compiler-perspective/en');
  const sigchip = page.getByRole('button', { name: 'Signature of this page' });
  await expect(sigchip).toBeVisible({ timeout: 10000 });
  await sigchip.click();
  await expect(page.locator('body')).toContainText('/writing/zk-proofs-from-a-compiler-perspective');
  await expect(page.locator('body')).toContainText('OpenPGP');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('content directories render as filtered ledger pages', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    dirEntry('projects', 'projects'),
    dirEntry('writing', 'writing'),
    fileEntry('projects/websh.md', 'websh', {
      size: 148,
      date: '2026-04-22',
      tags: ['rust']
    }),
    fileEntry('writing/content-backed-homepage.md', 'content-backed homepage', {
      size: 913,
      date: '2026-04-20',
      tags: ['notes']
    })
  ]);
  const ledger = makeLedger([
    makeLedgerEntry({
      route: '/projects/websh',
      path: 'projects/websh.md',
      date: '2026-04-22',
      files: [
        {
          path: 'content/projects/websh.md',
          sha256: normalizedSha('b'),
          bytes: 148
        }
      ]
    }),
    makeLedgerEntry({
      route: '/writing/content-backed-homepage',
      path: 'writing/content-backed-homepage.md',
      date: '2026-04-20',
      files: [
        {
          path: 'content/writing/content-backed-homepage.md',
          sha256: normalizedSha('a'),
          bytes: 913
        }
      ]
    })
  ]);
  const projectBlock = ledger.blocks.find((block) => block.entry.path === 'projects/websh.md');
  const writingBlock = ledger.blocks.find((block) => block.entry.path === 'writing/content-backed-homepage.md');

  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/.websh/ledger.json', JSON.stringify(ledger));

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/writing`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('~/writing', { timeout: 10000 });
  await expect(page.getByRole('link', { name: /^writing 1$/ })).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('region', { name: 'Ledger metadata' }).locator(`[aria-label="chain head ${ledger.chain_head}"]`)).toHaveCount(1);
  await expect(page.locator('article')).toHaveCount(1);
  const writingArticle = page.locator('article').first();
  await expect(writingArticle).toContainText('content-backed homepage');
  await expect(writingArticle).toContainText('block 0001');
  await expect(writingArticle.locator(`[aria-label="previous block hash ${writingBlock.prev_block_sha256}"]`)).toHaveCount(1);
  await expect(writingArticle.locator(`[aria-label="block hash ${writingBlock.block_sha256}"]`)).toHaveCount(1);
  await expect(writingArticle.locator('[aria-label="hash ok"]')).toHaveCount(1);
  await expect(page.locator('article').first()).not.toContainText('websh');

  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).not.toContainText('/home/j/ledger');
  await expect(page.locator('body')).not.toContainText('ledger A');
  await expect(page.getByRole('link', { name: /^all 2$/ })).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('region', { name: 'Ledger metadata' }).locator('[aria-label="hash ok"]')).toHaveCount(1);
  await expect(page.getByRole('region', { name: 'Ledger metadata' }).locator(`[aria-label="chain head ${ledger.chain_head}"]`)).toHaveCount(1);
  await expect(page.getByRole('region', { name: 'Ledger metadata' })).not.toContainText('verified');
  await expect(page.locator('article').first()).toContainText('websh');
  await expect(page.locator('article').first()).toContainText('block 0002');
  await expect(page.locator('article').first().locator(`[aria-label="block hash ${projectBlock.block_sha256}"]`)).toHaveCount(1);
  await expect(page.locator('article').filter({ hasText: 'content-backed homepage' })).toHaveCount(1);
  await expect(page.locator('article').filter({ hasText: 'websh' })).toHaveCount(1);

  await page.goto(`${baseUrl}/#/misc`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('~/misc');
  await expect(page.locator('body')).toContainText('no blocks match this ledger filter');
  await expect(page.locator('body')).not.toContainText('No route matched');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('ledger links site support block to directory listing', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    dirEntry('.site/errors', 'errors'),
    dirEntry('.site/keys', 'keys'),
    fileEntry('.site/errors/404.md', '404 response policy'),
    fileEntry('.site/keys/wonjae.asc', 'wonjae', { kind: 'document' }),
    fileEntry('.site/now.toml', 'now', { kind: 'document' })
  ]);
  const ledger = makeLedger([
    makeLedgerEntry({
      route: '/.site',
      path: '.site',
      date: null,
      files: [
        { path: 'content/.site/_index.dir.json', sha256: normalizedSha('a'), bytes: 300 },
        { path: 'content/.site/errors/404.md', sha256: normalizedSha('b'), bytes: 600 },
        { path: 'content/.site/errors/404.meta.json', sha256: normalizedSha('c'), bytes: 120 },
        { path: 'content/.site/errors/_index.dir.json', sha256: normalizedSha('d'), bytes: 80 },
        { path: 'content/.site/keys/_index.dir.json', sha256: normalizedSha('e'), bytes: 80 },
        { path: 'content/.site/keys/wonjae.asc', sha256: normalizedSha('f'), bytes: 640 },
        { path: 'content/.site/keys/wonjae.meta.json', sha256: normalizedSha('1'), bytes: 120 },
        { path: 'content/.site/now.meta.json', sha256: normalizedSha('2'), bytes: 120 },
        { path: 'content/.site/now.toml', sha256: normalizedSha('3'), bytes: 298 }
      ]
    })
  ]);

  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/.websh/ledger.json', JSON.stringify(ledger));

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'networkidle' });

  await expect(page.getByRole('link', { name: /^all 1$/ })).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('link', { name: /^misc 1$/ })).toHaveCount(1);
  const siteBlock = page.locator('article').filter({ hasText: 'Site support' });
  await expect(siteBlock).toHaveCount(1);
  await expect(siteBlock.locator('[data-kind="directory"]')).toHaveText('directory');
  await expect(siteBlock.locator('[aria-label="Bundle variants"]')).toHaveCount(0);
  await expect(siteBlock).not.toContainText('browse payload');

  await siteBlock.getByRole('link', { name: 'Site support' }).click();
  await page.waitForURL('**/#/.site');
  await expect(page.locator('body')).toContainText('~/.site');
  await expect(page.locator('body')).toContainText('directory');
  const metadata = page.locator('[aria-label="directory metadata"]');
  await expect(metadata).not.toContainText('~/.site');
  await expect(metadata).not.toContainText('directory_listing');
  await expect(metadata).not.toContainText('Description');
  await expect(metadata).toContainText('Tags');
  await expect(metadata).toContainText('runtime');
  await expect(metadata).toContainText('trust');
  await expect(metadata).toContainText('3 items');
  await expect(page.locator('body')).toContainText('Runtime support and trust metadata for the site.');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('now.toml');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('keys');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('errors');
  await expect(page.locator('body')).not.toContainText('no blocks match this ledger filter');
  await expect(page.locator('body')).not.toContainText('_index.dir.json');
  await expect(page.locator('body')).not.toContainText('now.meta.json');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('runtime state directory route renders the system filesystem listing', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/.websh/state`, { waitUntil: 'networkidle' });

  await expect(page.locator('body')).toContainText('~/.websh/state', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('session');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).toContainText('wallet');
  await expect(page.getByRole('navigation', { name: 'Directory entries' })).not.toContainText('drafts');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('theme selection applies globally and persists', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'kanagawa-wave');

  await page.getByRole('button', { name: /palette/i }).click();
  await page.getByRole('button', { name: /Black Ink/i }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'black-ink');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), themeStorageKey)).toBe('black-ink');

  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'black-ink');

  await page.getByRole('button', { name: /palette/i }).click();
  await page.getByRole('button', { name: /Sepia Dark/i }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'sepia-dark');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), themeStorageKey)).toBe('sepia-dark');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('unsupported command input never reaches output, history, storage, or requests', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  const requests = [];
  page.on('request', request => requests.push(request.url() + JSON.stringify(request.headers()) + (request.postData() || '')));
  await page.addInitScript(() => {
    localStorage.setItem('user.LANG', 'ko');
  });
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');
  for (const command of ['sync auth set credential-sentinel', '  SyNc   AUTH  SET credential-sentinel  ', 'echo hi | sync auth set credential-sentinel']) {
    await runCommand(page, command);
    await expect(page.locator('body')).toContainText('Unsupported command or syntax.');
    await expect(page.locator('body')).not.toContainText('credential-sentinel');
    await page.keyboard.press('ArrowUp');
    await expect(page.locator('input[type="text"]')).not.toHaveValue(/credential-sentinel/);
  }
  expect(await page.evaluate(() => localStorage.getItem('user.LANG'))).toBe('ko');
  expect(requests.join(' ')).not.toMatch(/credential-sentinel/);
  expect(consoleErrors.join(' ')).not.toMatch(/credential-sentinel/);
  expect(pageErrors).toEqual([]);
});

test('history replay retains executed arguments across repeated references', async ({ page }) => {
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');
  const repeated = page.getByText('history-result', { exact: true });
  await runCommand(page, 'echo history-result');
  await expect(repeated).toHaveCount(1);
  await runCommand(page, '!!');
  await expect(repeated).toHaveCount(2);
  await runCommand(page, '!!');
  await expect(repeated).toHaveCount(3);

  await runCommand(page, `export MESSAGE="literal \\$VALUE | !!"`);
  await runCommand(page, 'echo "$MESSAGE"');
  const literal = page.getByText('literal $VALUE | !!', { exact: true });
  await expect(literal).toHaveCount(1);
  await runCommand(page, '!!');
  await expect(literal).toHaveCount(2);
  await runCommand(page, '!-1');
  await expect(literal).toHaveCount(3);
  await page.keyboard.press('ArrowUp');
  await expect(page.locator('input[type="text"]')).toHaveValue("echo 'literal $VALUE | !!'");
});

test('wallet sessions remain usable but writing commands and redirection cannot edit content', async ({ page }) => {
  const mutations = [];
  page.on('request', request => { if (!['GET', 'HEAD', 'OPTIONS'].includes(request.method()) || /api\.github\.com|graphql/i.test(request.url()) || request.headers().authorization) mutations.push(request.url()); });
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await runCommand(page, 'login', 'Connected:');
  await runCommand(page, 'echo harmless > phantom.md');
  await runCommand(page, 'echo more >> phantom.md');
  for (const command of ['touch phantom.md', 'mkdir phantom', 'rm docs/old.md', 'rmdir docs', 'edit docs/old.md', 'sync commit removed']) await runCommand(page, command);
  await runCommand(page, 'cat phantom.md', 'No such file or directory');
  await runCommand(page, 'cat docs/old.md');
  await expect(page.locator('body')).toContainText('old');
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Connected:');
  await runCommand(page, 'logout');
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');
  expect(mutations).toEqual([]);
});

test('new is an ordinary content route and the reader has no editing controls', async ({ page }) => {
  await page.goto(`${baseUrl}/#/new`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: /not found/i })).toBeVisible();
  installContentPage('new.md', 'Ordinary New', '# Ordinary New');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: 'Ordinary New' }).first()).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Markdown source' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: /^(edit|save|compose)$/i })).toHaveCount(0);
});

test('warm external cache serves a known listing during failed refresh and retries in place', async ({ page }) => {
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(1);
  const gate = deferred();
  let offline = true;
  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', async route => {
    if (offline) { await gate.promise; await route.fulfill({ status: 503, body: 'offline' }); }
    else await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(dbManifest) });
  });
  await page.reload({ waitUntil: 'domcontentloaded' });
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  await expect(page.locator('[data-mount-status]')).toContainText('Saved listing');
  gate.resolve();
  await expect(page.locator('[data-mount-status]')).toContainText('Refresh failed');
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  offline = false;
  await page.getByRole('button', { name: 'Refresh listing' }).click();
  await expect(page.locator('[data-mount-status]')).toHaveCount(0);
});

test('a route absent from cached external metadata stays pending until the live answer', async ({ page }) => {
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(1);
  const gate = deferred();
  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', async route => {
    await gate.promise;
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(dbManifest) });
  });
  await page.goto(`${baseUrl}/#/db/not-in-listing`, { waitUntil: 'domcontentloaded' });
  await page.reload({ waitUntil: 'domcontentloaded' });
  await expect(page.getByRole('heading', { name: 'Route pending' })).toBeVisible();
  await expect(page.getByRole('heading', { name: /not found/i })).toHaveCount(0);
  gate.resolve();
  await expect(page.getByRole('heading', { name: /not found/i })).toBeVisible();
});

test('unavailable IndexedDB does not block external public reading', async ({ page }) => {
  await page.addInitScript(() => { IDBFactory.prototype.open = () => { throw new DOMException('storage denied', 'SecurityError'); }; });
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  await expect(page.locator('body')).not.toContainText('storage denied');
});

for (const scenario of ['missing', 'rejected']) {
  test(`wallet ${scenario} is graceful and leaves content readable`, async ({ page }) => {
    await page.addInitScript((scenario) => {
      if (scenario === 'missing') delete window.ethereum;
      else window.ethereum.request = async () => { throw new Error('user declined'); };
    }, scenario);
    await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
    await runCommand(page, 'login');
    await expect(page.locator('body')).toContainText(scenario === 'missing' ? 'no wallet provider detected' : 'user declined');
    await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
    await expect(page.locator('[data-reader-body]')).toContainText('old');
  });
}

test('wallet account and chain events update identity without repartitioning public cache', async ({ page }) => {
  await page.addInitScript((address) => {
    const listeners = new Map();
    window.__walletListeners = listeners;
    window.__walletEmit = (event, value) => (listeners.get(event) || []).forEach(fn => fn(value));
    window.ethereum = {
      request: async ({ method }) => method === 'eth_chainId' ? '0x1' : [address],
      on: (event, listener) => listeners.set(event, [...(listeners.get(event) || []), listener]),
      removeListener: (event, listener) => listeners.set(event, (listeners.get(event) || []).filter(fn => fn !== listener))
    };
  }, walletAddress);
  await page.route('https://api.ensideas.com/**', route => route.fulfill({ status: 503, body: 'ENS unavailable' }));
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await runCommand(page, 'login', 'Connected:');
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(1);
  const keys = (await cacheRecords(page)).map(record => record.key);
  const second = '0x1111111111111111111111111111111111111111';
  await page.evaluate(second => { window.__walletEmit('accountsChanged', [second]); window.__walletEmit('chainChanged', '0x89'); }, second);
  await runCommand(page, 'id', 'chain_id=137');
  await expect(page.locator('body')).toContainText(second);
  await runCommand(page, 'cat /.websh/state/wallet/connection.json');
  await expect(page.locator('body')).toContainText(second);
  expect((await cacheRecords(page)).map(record => record.key)).toEqual(keys);
  expect(await page.evaluate(() => Array.from(window.__walletListeners, ([event, callbacks]) => [event, callbacks.length]))).toEqual([['accountsChanged', 1], ['chainChanged', 1]]);
  await page.evaluate(() => window.__walletEmit('accountsChanged', []));
  await expect(page.locator('[data-reader-body]')).toContainText('Disconnected');
  expect(await page.evaluate(() => localStorage.getItem('websh.wallet_session'))).toBeNull();
});

test('cached metadata never invents an unavailable body or repairs a failed cold root', async ({ page }) => {
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(1);
  const records = await cacheRecords(page);
  expect(records.every(record => !record.descriptor.root.startsWith('/.websh'))).toBe(true);
  expect(JSON.stringify(records)).not.toMatch(/# Fresh|websh\.wallet_session|gh_token|attestation_verdict/);
  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', route => route.fulfill({ status: 503, body: 'offline' }));
  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/fresh.md', route => route.fulfill({ status: 404, body: 'gone' }));
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body]')).toContainText('not found');
  await page.route('**/content/manifest.json', route => route.fulfill({ status: 503, body: 'root offline' }));
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: 'Route unconfirmed' })).toBeVisible();
  await expect(page.locator('[data-reader-body]')).toHaveCount(0);
});


test('two tabs keep the newer started mount observation when an older refresh finishes last', async ({ page, context }) => {
  const mountUrl = 'https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json';
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(1);
  const gate = deferred();
  const started = deferred();
  const older = manifestDocument([dirEntry('', 'DB'), fileEntry('fresh.md', 'Older response')]);
  const newer = manifestDocument([dirEntry('', 'DB'), fileEntry('fresh.md', 'Newer response')]);
  await page.route(mountUrl, async route => { started.resolve(); await gate.promise; await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(older) }); });
  await page.goto(`${baseUrl}/#/websh`);
  await runCommand(page, 'refresh /db');
  await started.promise;
  const second = await context.newPage();
  const serveFixture = async route => {
    const url = new URL(route.request().url());
    const body = url.href === mountUrl ? JSON.stringify(newer) : rawResponses.get(fixturePathname(url));
    await route.fulfill({ status: body === undefined ? 404 : 200, contentType: contentTypeForPath(url.pathname), body: body === undefined ? 'missing' : body });
  };
  await second.route('**/content/**', serveFixture);
  await second.route('https://raw.githubusercontent.com/**', serveFixture);
  await second.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect.poll(async () => (await cacheRecords(second))[0]?.manifest_json).toContain('Newer response');
  const accepted = (await cacheRecords(second))[0];
  gate.resolve();
  await expect(page.locator('body')).toContainText('reloaded');
  // Wait for the first tab's cache compare transaction, not just its visible publication.
  await expect.poll(async () => (await cacheRecords(page))[0]?.request_started_at_ms).toBe(accepted.request_started_at_ms);
  await page.waitForTimeout(250);
  expect((await cacheRecords(page))[0].manifest_json).toContain('Newer response');
  await second.close();
});

test('404 and malformed refresh preserve the current body; a valid empty listing confirms removal', async ({ page }) => {
  const mountUrl = 'https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json';
  let bodyReads = 0;
  page.on('request', request => { if (request.url().endsWith('/mount-db/main/fresh.md')) bodyReads++; });
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  const initialReads = bodyReads;
  const before = (await cacheRecords(page))[0];
  let response = { status: 404, body: 'missing manifest' };
  await page.route(mountUrl, route => route.fulfill(response));
  await page.goto(`${baseUrl}/#/websh`);
  await runCommand(page, 'refresh /db');
  await expect(page.locator('body')).toContainText('not found');
  await page.goto(`${baseUrl}/#/db/fresh`);
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  await expect(page.locator('[data-mount-status]')).toContainText('Refresh failed');
  response = { status: 200, contentType: 'application/json', body: '{invalid' };
  await page.getByRole('button', { name: 'Refresh listing' }).click();
  await expect(page.locator('[data-mount-status]')).toContainText('Refresh failed');
  expect(bodyReads).toBe(initialReads);
  expect((await cacheRecords(page))[0].manifest_json).toBe(before.manifest_json);
  response = { status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [] }) };
  await page.getByRole('button', { name: 'Refresh listing' }).click();
  await expect(page.getByRole('heading', { name: /not found/i })).toBeVisible();
});
