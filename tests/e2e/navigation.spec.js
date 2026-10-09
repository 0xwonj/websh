const { test, expect, baseUrl, rawOrigin, rootPath, rootPointer, siteEntries, fileEntry, dirEntry, home, publishRoot, installContentPage } = require('./support/fixtures');
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

test('cold home authenticates three root requests without bundled content', async ({ page, request, responses }) => {
  for (const path of ['/assets/manifest.json','/assets/favicon.svg','/assets/crypto/site.asc']) expect((await request.get(`${baseUrl}${path}`)).status()).toBe(200);
  expect((await request.get(`${baseUrl}/content/manifest.json`)).status()).toBe(404);
  const network=collectNavigationNetwork(page); const requests=[];
  page.on('request',request=>{ if(request.url().startsWith(`${rawOrigin}/0xwonj/websh-content/`)) requests.push(new URL(request.url()).pathname); });
  await page.addInitScript(() => {
    const observer = new MutationObserver(() => {
      if (document.body?.textContent.includes('Fixture Now item')) {
        window.fixtureHomeReady = performance.now();
        observer.disconnect();
      }
    });
    observer.observe(document, {subtree: true, childList: true});
  });
  await page.goto(`${baseUrl}/`,{waitUntil:'networkidle'});
  await expect(page.locator('body')).toContainText('A Homepage, Formalised');
  await expect(page.getByRole('navigation',{name:'path'})).toHaveText('~');
  expect(requests.sort()).toEqual([rootPointer,rootPath('manifest.json'),rootPath('manifest.sig')].sort());
  const timing = await page.evaluate(() => {
    const resources = performance.getEntriesByType('resource').filter(entry => entry.name.includes('/0xwonj/websh-content/'));
    const firstRequest = Math.min(...resources.map(entry => entry.startTime));
    const lastResponse = Math.max(...resources.map(entry => entry.responseEnd));
    return {
      rootRequests: resources.length,
      discoveryMs: Math.round(lastResponse - firstRequest),
      verifyAndInstallMs: Math.round(window.fixtureHomeReady - lastResponse),
      homeReadyMs: Math.round(window.fixtureHomeReady)
    };
  });
  const metadataBytes = [rootPointer, rootPath('manifest.json'), rootPath('manifest.sig')]
    .reduce((total, path) => total + Buffer.byteLength(responses.get(path)), 0);
  console.log(`Cold fixture home: ${JSON.stringify({...timing, metadataBytes})}`);
  await expect(page.getByRole('tooltip')).toHaveCount(0);
  await expect(page.locator('a[href*="?content="]')).toHaveCount(0);
  const signature = page.getByRole('button',{name:'Signature of this page'});
  await signature.click();
  const details = page.getByRole('tooltip');
  await expect(details).toContainText('OpenPGP · detached signature');
  await expect(details).toContainText('-----BEGIN PGP SIGNATURE-----');
  await expect(details).toContainText('Fixture Author <fixture@example.test>');
  await expect(details).toContainText('SHA256(home @ /) = 0x');
  expect(await details.locator('[class*="sigK"]').allTextContents()).toEqual([
    'route', 'content', 'ack root', 'signed by', 'fingerprint', 'scheme', 'message', 'verified', 'snapshot'
  ]);
  await expect(details.locator('[class*="sigRow"]').filter({hasText: /^verified /})).toHaveText(/verified 0x[0-9a-f]+…[0-9a-f]+ ✓/);
  const snapshot = details.getByRole('link');
  await expect(snapshot).toHaveAttribute('href', /\?content=[a-f0-9]{40}&release=[a-f0-9]{64}#\/$/);
  await expect(snapshot).toHaveText(await snapshot.getAttribute('href'));
  await signature.press('Escape');
  await expect(details).toHaveCount(0);
  expect(network.sameOriginFailures).toEqual([]);
});

test('home projection appears atomically after signature and manifest verification', async ({page,responses})=>{
 const gate=deferred();const started=deferred();
 publishRoot(responses,{entries:[...siteEntries,dirEntry('writing','writing'),fileEntry('writing/loaded.md','Loaded Writing',{date:'2026-05-01'})],files:{'writing/loaded.md':'# Loaded'},projection:{...home,now:{items:[{date:'2026-05-01',text:'Loaded now item'}]}}});
 await page.route(`${rawOrigin}${rootPath('manifest.sig')}`,async route=>{started.resolve();await gate.promise;await route.fulfill({status:200,body:responses.get(rootPath('manifest.sig'))});});
 await page.goto(`${baseUrl}/#/`,{waitUntil:'domcontentloaded'});await started.promise;
 await expect(page.locator('body')).not.toContainText('Loaded now item');
 await expect(page.locator('body')).not.toContainText('Fixture Author');
 gate.resolve();
 await expect(page.locator('body')).toContainText('Loaded now item');
 await expect(page.getByRole('navigation',{name:'Site index'}).getByRole('link',{name:/writing/})).toContainText('1');
});

test('missing routes wait for an authenticated catalog and failures never become 404', async ({page,responses,errors})=>{
 const gate=deferred();const started=deferred();
 await page.route(`${rawOrigin}${rootPath('manifest.json')}`,async route=>{started.resolve();await gate.promise;await route.fulfill({status:200,body:responses.get(rootPath('manifest.json'))});});
 await page.goto(`${baseUrl}/#/docs/missing`,{waitUntil:'domcontentloaded'});await started.promise;
 await expect(page.getByRole('heading',{name:'Route pending'})).toBeVisible();
 gate.resolve();await expect(page.getByRole('heading',{name:/not found/i})).toBeVisible();
});

test('cold invalid root signature is rejected before homepage publication',async ({page,responses})=>{
 responses.set(rootPath('manifest.sig'),'not a signature');
 await page.goto(`${baseUrl}/#/`,{waitUntil:'networkidle'});
 await expect(page.locator('[data-mount-status]')).toContainText(/failed|unavailable/i);
 await expect(page.locator('body')).not.toContainText('Fixture Author');
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
