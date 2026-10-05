const { createHash } = require('node:crypto');
const { test, expect, baseUrl, siteManifest, fileEntry, dirEntry, manifestDocument, installContentPage } = require('./support/fixtures');
const { deferred, collectNavigationNetwork, installIpfsBaseAlias } = require('./support/browser');

async function readBreadcrumbLayout(page) {
  return page.evaluate(() => {
    const breadcrumb = document.querySelector('[data-chrome-role="breadcrumb"]');
    if (!breadcrumb?.querySelector('[data-breadcrumb-current]')) return null;
    const blockers = Array.from(document.querySelectorAll('[data-chrome-role]'))
      .filter(element => element !== breadcrumb)
      .map(element => ({ role: element.dataset.chromeRole, rect: element.getBoundingClientRect() }));
    const blockedBy = rect => blockers.filter(({ rect: other }) =>
      rect.top < other.bottom && rect.bottom > other.top &&
      rect.left < other.right && rect.right > other.left
    ).map(({ role }) => role);
    const crumbs = Array.from(breadcrumb.querySelectorAll(':scope > a, :scope > [data-breadcrumb-current]'));
    return {
      crumbs: crumbs.map(element => ({
        text: element.textContent.trim(),
        clientWidth: element.clientWidth,
        scrollWidth: element.scrollWidth
      })),
      overlaps: blockedBy(breadcrumb.getBoundingClientRect()),
      crumbOverlaps: crumbs.flatMap(element => blockedBy(element.getBoundingClientRect())),
      scrollWidth: document.documentElement.scrollWidth,
      viewportWidth: window.innerWidth
    };
  });
}

test('root loads the built-in homepage and public app assets', async ({ page, request }) => {
  for (const path of ['/assets/manifest.json', '/assets/favicon.svg']) {
    expect((await request.get(`${baseUrl}${path}`)).status(), path).toBe(200);
  }
  const response = await request.get(`${baseUrl}/assets/crypto/attestations.json`);
  expect(response.status()).toBe(200);
  const artifact = await response.json();
  const publicFiles = artifact.subjects.flatMap(subject => subject.content_files)
    .filter(file => file.path.startsWith('assets/'));
  expect(publicFiles.length).toBeGreaterThan(0);
  for (const file of publicFiles) {
    const response = await request.get(`${baseUrl}/${file.path}`);
    expect(response.status(), file.path).toBe(200);
    const bytes = await response.body();
    expect(bytes.length, file.path).toBe(file.bytes);
    expect(`0x${createHash('sha256').update(bytes).digest('hex')}`, file.path).toBe(file.sha256);
  }
  const network = collectNavigationNetwork(page);
  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  expect(new URL(page.url()).hash).toBe('#/');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~');
  await expect(page.locator('body')).not.toContainText('No route matched');
  expect(network.sameOriginFailures).toEqual([]);
});

test('home renders static sections while the root manifest is still loading', async ({ page, responses }) => {
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
  responses.set('/content/manifest.json', JSON.stringify(manifest));
  responses.set('/content/.site/now.toml', '[[items]]\ndate = "2026-05-01"\ntext = "Loaded now item"\n');

  await page.route('**/content/manifest.json', async (route) => {
    manifestRequested.resolve();
    await releaseManifest.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: responses.get('/content/manifest.json')
    });
  });

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
});

test('missing content hash route shows 404 only after root manifest loads', async ({ page, responses }) => {
  const manifestRequested = deferred();
  const releaseManifest = deferred();

  await page.route('**/content/manifest.json', async (route) => {
    manifestRequested.resolve();
    await releaseManifest.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: responses.get('/content/manifest.json')
    });
  });

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
});

test('direct content hash route reports root mount failure when manifest fails', async ({ page, errors }) => {
  errors.allowHttpError('/content/manifest.json', 404);
  await page.route('**/content/manifest.json', async (route) => {
    await route.fulfill({
      status: 404,
      contentType: 'text/plain',
      body: 'root manifest unavailable'
    });
  });

  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'domcontentloaded' });

  await expect(page.locator('body')).toContainText(/route unconfirmed/i, { timeout: 10000 });
  await expect(page.locator('body')).toContainText('content/manifest.json');
  await expect(page.locator('body')).not.toContainText('404');
  await expect(page.locator('body')).not.toContainText('No route matched');
});

test('ledger navigation shares the home prefetch request', async ({ page, responses }) => {
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
      body: responses.get('/content/.websh/ledger.json')
    });
  });

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
});

test('long breadcrumbs remain readable without colliding with responsive navigation', async ({ page, responses }) => {
  const path = 'writing/research/compiler-notes/zkvm-performance/a-very-long-current-title-that-would-otherwise-collide-with-navigation.md';
  installContentPage(responses, path, 'Nested Long Crumb', '# Nested long crumb');
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto(`${baseUrl}/#/${path.replace(/\.md$/, '')}`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Nested long crumb');

  for (const width of [360, 560, 900, 1280]) {
    await page.setViewportSize({ width, height: 720 });
    await expect(async () => {
      const layout = await readBreadcrumbLayout(page);
      expect(layout).not.toBeNull();
      expect(layout.crumbs).toHaveLength(6);
      if (width < 1280) {
        expect(layout.crumbs.some(crumb => crumb.scrollWidth > crumb.clientWidth)).toBe(true);
      }
      expect(layout.overlaps).toEqual([]);
      expect(layout.crumbOverlaps).toEqual([]);
      expect(layout.scrollWidth).toBeLessThanOrEqual(layout.viewportWidth);
    }).toPass({ timeout: 10000 });
  }
});

test('home navigation stays inside the hash router', async ({ page }) => {
  const network = collectNavigationNetwork(page);

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
});

test('ipfs base navigation preserves the hash-router base', async ({ page }) => {
  await installIpfsBaseAlias(page);
  const network = collectNavigationNetwork(page);

  await page.goto(`${baseUrl}/ipfs/fakecid/#/ledger`, { waitUntil: 'networkidle' });
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
});
