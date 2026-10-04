const { test, expect, baseUrl, dbManifest, fileEntry, dirEntry, manifestDocument, installRoutes } = require('./support/fixtures');
const { deferred, runCommand, cacheRecords } = require('./support/browser');

test('root content renders before external mount scan resolves', async ({ page }) => {
  const gate = deferred();

  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', async (route) => {
    await gate.promise;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(dbManifest)
    });
  });

  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'domcontentloaded' });
  await expect(page.locator('body')).toContainText('old', { timeout: 10000 });

  gate.resolve();
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Fresh', { timeout: 10000 });
});

test('warm external cache serves a known listing during failed refresh and retries in place', async ({ page, errors }) => {
  errors.allowHttpError('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', 503);
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

test('a saved listing cannot supply an unavailable content body', async ({ page, errors }) => {
  errors.allowHttpError('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', 503);
  errors.allowHttpError('https://raw.githubusercontent.com/0xwonj/mount-db/main/fresh.md', 404);
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(1);
  const records = await cacheRecords(page);
  expect(records.every(record => !record.descriptor.root.startsWith('/.websh'))).toBe(true);
  expect(JSON.stringify(records)).not.toContain('# Fresh');
  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json', route => route.fulfill({ status: 503, body: 'offline' }));
  await page.route('https://raw.githubusercontent.com/0xwonj/mount-db/main/fresh.md', route => route.fulfill({ status: 404, body: 'gone' }));
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body]')).toContainText('not found');
});

test('two tabs keep the newer started mount observation when an older refresh finishes last', async ({ page, context, responses }) => {
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
  const secondResponses = new Map(responses);
  secondResponses.set('/0xwonj/mount-db/main/manifest.json', JSON.stringify(newer));
  await installRoutes(second, secondResponses);
  await second.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect.poll(async () => (await cacheRecords(second))[0]?.manifest_json).toContain('Newer response');
  const accepted = (await cacheRecords(second))[0];
  await page.evaluate(() => {
    window.__cacheTransactionDone = false;
    const transaction = IDBDatabase.prototype.transaction;
    IDBDatabase.prototype.transaction = function (...args) {
      const tx = transaction.apply(this, args);
      if (this.name === 'websh-cache' && args[1] === 'readwrite') {
        tx.addEventListener('complete', () => { window.__cacheTransactionDone = true; });
      }
      return tx;
    };
  });
  gate.resolve();
  await expect(page.locator('body')).toContainText('reloaded');
  // Wait for the first tab's cache compare transaction, not just its visible publication.
  await expect.poll(() => page.evaluate(() => window.__cacheTransactionDone)).toBe(true);
  expect((await cacheRecords(page))[0].request_started_at_ms).toBe(accepted.request_started_at_ms);
  expect((await cacheRecords(page))[0].manifest_json).toContain('Newer response');
  await second.close();
});

test('an accepted empty listing removes the current external route', async ({ page }) => {
  const mountUrl = 'https://raw.githubusercontent.com/0xwonj/mount-db/main/manifest.json';
  await page.goto(`${baseUrl}/#/db/fresh`, { waitUntil: 'networkidle' });
  await expect(page.getByRole('heading', { name: 'Fresh', exact: true }).first()).toBeVisible();
  await page.route(mountUrl, route => route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify({ entries: [] })
  }));
  await page.goto(`${baseUrl}/#/websh`);
  await runCommand(page, 'refresh /db', 'reloaded');
  await page.goto(`${baseUrl}/#/db/fresh`);
  await expect(page.getByRole('heading', { name: /not found/i })).toBeVisible();
});
