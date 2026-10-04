const { test, expect, baseUrl, siteManifest, fileEntry, dirEntry, manifestDocument } = require('./support/fixtures');
const { makeLedger, makeLedgerEntry, normalizedSha } = require('./support/ledger');

test('content directories render as filtered ledger pages', async ({ page, responses }) => {
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

  responses.set('/content/manifest.json', JSON.stringify(manifest));
  responses.set('/content/.websh/ledger.json', JSON.stringify(ledger));

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
});

test('ledger links site support block to directory listing', async ({ page, responses }) => {
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

  responses.set('/content/manifest.json', JSON.stringify(manifest));
  responses.set('/content/.websh/ledger.json', JSON.stringify(ledger));

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
});

test('mempool filters pending entries and opens the selected content route', async ({ page, responses }) => {
  responses.set('/content/manifest.json', JSON.stringify(manifestDocument([
    dirEntry('', 'Home'),
    dirEntry('.websh', '.websh'),
    dirEntry('.websh/mounts', 'mounts'),
    fileEntry('.websh/ledger.json', 'Ledger'),
    fileEntry('.websh/mounts/mempool.mount.json', 'Mempool mount')
  ])));
  responses.set('/content/.websh/mounts/mempool.mount.json', JSON.stringify({
    backend: 'github', mount_at: '/mempool', repo: '0xwonj/websh-mempool',
    branch: 'main', root: '', name: 'mempool'
  }));
  responses.set('/0xwonj/websh-mempool/main/manifest.json', JSON.stringify(manifestDocument([
    dirEntry('', 'Mempool'),
    ...['writing', 'projects'].map(category => ({
      ...fileEntry(`${category}/fixture.md`, `${category} entry`),
      mempool: { status: 'review', priority: 'high', category }
    }))
  ])));
  responses.set('/0xwonj/websh-mempool/main/writing/fixture.md', '# Pending writing\n\nFixture body.');

  const mempool = page.getByRole('region', { name: 'Mempool — pending blocks' });
  const open = async () => {
    const toggle = mempool.getByRole('button', { name: /mempool/i });
    if (await toggle.getAttribute('aria-expanded') !== 'true') await toggle.click();
  };
  await page.goto(`${baseUrl}/#/ledger`, { waitUntil: 'networkidle' });
  await open();
  await expect(mempool.locator('a [data-kind]')).toHaveCount(2);
  await page.goto(`${baseUrl}/#/writing`, { waitUntil: 'networkidle' });
  await open();
  await expect(mempool.locator('a [data-kind]')).toHaveText(['writing']);
  await expect(mempool).toContainText('1 / 2 pending');
  await mempool.getByRole('link', { name: /writing entry/ }).click();
  await page.waitForURL('**/#/mempool/writing/fixture');
  await expect(page.locator('[data-reader-body]')).toContainText('Fixture body.');
});
