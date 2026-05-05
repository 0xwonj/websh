// Mempool — Phase 1 visual QA, updated in Phase 6 for URL-driven flows.
//
// Default tests use local route fixtures. Set WEBSH_LIVE_MEMPOOL=1 to run the
// same assertions against the configured live mempool mount.

const { test, expect } = require('playwright/test');

const baseUrl = process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173';
const mempoolRegion = page => page.getByRole('region', { name: 'Mempool — pending blocks' });
const useLiveMempool = process.env.WEBSH_LIVE_MEMPOOL === '1';
const genesisHash = '0x0000000000000000000000000000000000000000000000000000000000000000';

function nodeMetadata(kind, { title, renderer = null } = {}) {
  return {
    schema: 1,
    kind,
    authored: title ? { title } : {},
    derived: renderer ? { renderer } : {}
  };
}

const fixtureResponses = new Map([
  ['/content/manifest.json', JSON.stringify({
    entries: [
      { path: '', metadata: nodeMetadata('directory', { title: 'Home' }) },
      { path: '.websh', metadata: nodeMetadata('directory', { title: '.websh' }) },
      { path: '.websh/mounts', metadata: nodeMetadata('directory', { title: 'mounts' }) },
      { path: '.websh/index.json', metadata: nodeMetadata('data', { title: 'Index' }) },
      { path: '.websh/ledger.json', metadata: nodeMetadata('data', { title: 'Ledger' }) },
      { path: '.websh/mounts/mempool.mount.json', metadata: nodeMetadata('data', { title: 'Mempool mount' }) },
      { path: 'index.html', metadata: nodeMetadata('page', { title: 'Home', renderer: 'html_page' }) }
    ]
  })],
  ['/content/index.html', '<main><h1>Home OK</h1></main>'],
  ['/content/.websh/index.json', JSON.stringify({
    routes: [
      { route: '/', node_path: '/index.html', kind: 'page', renderer: 'html_page' }
    ]
  })],
  ['/content/.websh/ledger.json', JSON.stringify({
    version: 1,
    scheme: 'websh.content-ledger.v1',
    hash: 'sha256',
    genesis_hash: genesisHash,
    blocks: [],
    block_count: 0,
    chain_head: genesisHash
  })],
  ['/content/.websh/mounts/mempool.mount.json', JSON.stringify({
    backend: 'github',
    mount_at: '/mempool',
    repo: '0xwonj/websh-mempool',
    branch: 'main',
    root: '',
    name: 'mempool',
    writable: true
  })],
  ['/0xwonj/websh-mempool/main/manifest.json', JSON.stringify({
    entries: [
      {
        path: '',
        metadata: nodeMetadata('directory', { title: 'Mempool' })
      },
      {
        path: 'writing/fixture-entry.md',
        metadata: nodeMetadata('page', { title: 'Fixture Entry', renderer: 'markdown_page' }),
        mempool: { status: 'review', priority: 'high', category: 'writing' }
      }
    ]
  })],
  ['/0xwonj/websh-mempool/main/writing/fixture-entry.md', '---\ntitle: Fixture Entry\nstatus: review\nmodified: 2026-05-05\n---\n\nFixture body.\n']
]);

async function openMempool(page) {
  const mempool = mempoolRegion(page);
  await expect(mempool).toBeVisible();
  const toggle = mempool.getByRole('button', { name: /mempool/i });
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') {
    await toggle.click();
  }
  return mempool;
}

test.describe('mempool', () => {
  test.beforeEach(async ({ page }) => {
    if (useLiveMempool) {
      return;
    }

    await page.route('**/content/**', async (route) => {
      const url = new URL(route.request().url());
      const body = fixtureResponses.get(url.pathname);
      if (body === undefined) {
        await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: url.pathname.endsWith('.json') ? 'application/json' : 'text/plain',
        body
      });
    });

    await page.route('https://raw.githubusercontent.com/**', async (route) => {
      const url = new URL(route.request().url());
      const body = fixtureResponses.get(url.pathname);
      if (body === undefined) {
        await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
        return;
      }
      await route.fulfill({
        status: 200,
        contentType: url.pathname.endsWith('.json') ? 'application/json' : 'text/plain',
        body
      });
    });
  });

  test('renders above chain on /ledger with at least one entry', async ({ page }) => {
    await page.goto(`${baseUrl}/#/ledger`);
    const mempool = await openMempool(page);
    await expect(mempool.locator('a').first()).toBeVisible({ timeout: 15000 });
    const items = await mempool.locator('a').count();
    expect(items).toBeGreaterThan(0);
  });

  test('filter narrows mempool to category', async ({ page }) => {
    await page.goto(`${baseUrl}/#/writing`);
    const mempool = await openMempool(page);
    await expect(mempool.locator('a [data-kind]').first()).toBeVisible({ timeout: 15000 });
    const itemKinds = await mempool.locator('a [data-kind]').allTextContents();
    for (const kind of itemKinds) {
      expect(['writing']).toContain(kind);
    }
    // Header shows "X / Y pending"
    const headerText = await mempool.locator('span', { hasText: /pending/ }).innerText();
    expect(headerText).toMatch(/\d+ \/ \d+ pending/i);
  });

  test('clicking a row navigates to the entry view URL', async ({ page }) => {
    await page.goto(`${baseUrl}/#/ledger`);
    const initialHash = await page.evaluate(() => window.location.hash);
    const mempool = await openMempool(page);
    const firstRow = mempool.locator('a').first();
    await expect(firstRow).toBeVisible({ timeout: 15000 });
    const href = await firstRow.getAttribute('href');
    expect(href).toMatch(/^#\/mempool\//);
    await firstRow.click();
    const afterClickHash = await page.evaluate(() => window.location.hash);
    expect(afterClickHash).not.toBe(initialHash);
    expect(afterClickHash).toMatch(/^#\/mempool\//);
    // Phase 6 dropped the modal preview entirely.
    await expect(page.locator('[aria-label="Close preview"]')).toHaveCount(0);
  });
});
