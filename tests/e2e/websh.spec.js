const { test, expect } = require('playwright/test');
const crypto = require('crypto');

const baseUrl = process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173';
const admin = '0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a';
const expectedHead = '1111111111111111111111111111111111111111';
const themeStorageKey = 'user.THEME';

function nodeMetadata(kind, { title, description = null, date = null, tags = [], size = null, modified = null, access = null, renderer = null } = {}) {
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

  return {
    schema: 1,
    kind,
    authored,
    derived
  };
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
    metadata: nodeMetadata('directory', { title, ...options })
  };
}

function manifestDocument(entries) {
  return { entries };
}

const siteEntries = [
  dirEntry('', 'Home'),
  dirEntry('.websh', '.websh'),
  dirEntry('.websh/mounts', 'mounts'),
  dirEntry('docs', 'docs'),
  dirEntry('docs/deep', 'deep'),
  fileEntry('.websh/index.json', 'Index', { kind: 'data' }),
  fileEntry('.websh/mounts/db.mount.json', 'DB mount', { kind: 'data' }),
  fileEntry('.websh/site.json', 'Site', { kind: 'data' }),
  fileEntry('docs/deep/old.md', 'Deep Old'),
  fileEntry('docs/old.md', 'Old'),
  fileEntry('index.html', 'Home')
];

const siteManifest = manifestDocument(siteEntries);

const dbManifest = manifestDocument([
  dirEntry('', 'DB'),
  fileEntry('fresh.md', 'Fresh')
]);

let rawResponses;

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

function freshRawResponses() {
  return new Map([
    ['/content/manifest.json', JSON.stringify(siteManifest)],
    ['/content/index.html', '<main><h1>Home OK</h1></main>'],
    ['/content/docs/old.md', 'old'],
    ['/content/docs/deep/old.md', 'deep old'],
    ['/content/.websh/site.json', '{}'],
    ['/content/.websh/index.json', JSON.stringify({
      routes: [
        { route: '/', node_path: '/index.html', kind: 'page', renderer: 'html_page' }
      ]
    })],
    ['/content/.websh/mounts/db.mount.json', JSON.stringify({
      backend: 'github',
      mount_at: '/db',
      repo: '0xwonj/mount-db',
      branch: 'main',
      root: '',
      name: 'db',
      writable: true
    })],
    ['/0xwonj/mount-db/main/manifest.json', JSON.stringify(dbManifest)],
    ['/0xwonj/mount-db/main/fresh.md', '# Fresh']
  ]);
}

test.beforeEach(async ({ page }) => {
  rawResponses = freshRawResponses();

  await page.addInitScript((adminAddress) => {
    window.ethereum = {
      request: async ({ method }) => {
        if (method === 'eth_requestAccounts' || method === 'eth_accounts') {
          return [adminAddress];
        }
        if (method === 'eth_chainId') {
          return '0x1';
        }
        return null;
      }
    };
  }, admin);

  await page.route('https://api.ensideas.com/**', async (route) => {
    await route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
  });

  await page.route('**/content/**', async (route) => {
    const url = new URL(route.request().url());
    const body = rawResponses.get(url.pathname);
    if (body === undefined) {
      await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
      return;
    }
    const contentType = url.pathname.endsWith('.json')
      ? 'application/json'
      : url.pathname.endsWith('.pdf')
        ? 'application/pdf'
        : 'text/plain';
    await route.fulfill({ status: 200, contentType, body });
  });

  await page.route('https://raw.githubusercontent.com/**', async (route) => {
    const url = new URL(route.request().url());
    const body = rawResponses.get(url.pathname);
    if (body === undefined) {
      await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
      return;
    }
    const contentType = url.pathname.endsWith('.json')
      ? 'application/json'
      : url.pathname.endsWith('.pdf')
        ? 'application/pdf'
        : 'text/plain';
    await route.fulfill({ status: 200, contentType, body });
  });
});

async function collectBrowserErrors(page) {
  const pageErrors = [];
  const consoleErrors = [];
  page.on('pageerror', (error) => pageErrors.push(`${page.url()}: ${error.stack || error.message}`));
  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push(message.text());
    }
  });
  return { pageErrors, consoleErrors };
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

async function putMetadata(page, key, value) {
  await page.evaluate(([metadataKey, metadataValue]) => new Promise((resolve, reject) => {
    const request = indexedDB.open('websh-state', 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains('drafts')) {
        db.createObjectStore('drafts', { keyPath: 'mount_id' });
      }
      if (!db.objectStoreNames.contains('metadata')) {
        db.createObjectStore('metadata', { keyPath: 'key' });
      }
    };
    request.onerror = () => reject(request.error);
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction(['metadata'], 'readwrite');
      tx.objectStore('metadata').put({ key: metadataKey, value: metadataValue });
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onerror = () => reject(tx.error);
    };
  }), [key, value]);
}

async function waitForDraftPath(page, path) {
  await expect(async () => {
    const serialized = await page.evaluate((draftPath) => new Promise((resolve, reject) => {
      const request = indexedDB.open('websh-state', 1);
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        const db = request.result;
        const tx = db.transaction(['drafts'], 'readonly');
        const get = tx.objectStore('drafts').get('global');
        get.onsuccess = () => {
          db.close();
          resolve(JSON.stringify(get.result || {}));
        };
        get.onerror = () => reject(get.error);
      };
    }), path);
    expect(serialized).toContain(path);
  }).toPass({ timeout: 5000 });
}

const directLoadCases = [
  ['/#/', 'A Homepage, Formalised'],
  ['/#/index.html', 'Home OK'],
  ['/#/websh', 'guest@wonjae.eth:~'],
  ['/#/websh/db', '~/websh/db'],
  ['/#/db/fresh.md', 'Fresh']
];

test('official root loads built-in homepage', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  expect(new URL(page.url()).hash).toBe('');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~');
  await expect(page.locator('body')).not.toContainText('No route matched');
  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('official root does not require an index file in the mounted filesystem', async ({ page }) => {
  rawResponses = new Map([
    ['/content/manifest.json', JSON.stringify(manifestDocument([dirEntry('', 'Home')]))]
  ]);

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  expect(new URL(page.url()).hash).toBe('');
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.getByRole('navigation', { name: 'path' })).toHaveText('~');
  await expect(page.locator('body')).not.toContainText('No route matched');
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

test('pdf content renders through a blob-backed iframe', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    fileEntry('docs/sample.pdf', 'Sample PDF', { kind: 'document' })
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/docs/sample.pdf', Buffer.from('%PDF-1.4\n%%EOF\n'));

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/docs/sample.pdf`, { waitUntil: 'networkidle' });
  await expect(page.locator('iframe')).toHaveAttribute('src', /^blob:/, {
    timeout: 10000
  });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors.filter((message) => message.includes('Content Security Policy'))).toEqual([]);
});

test('attested renderer page shows the route sigchip', async ({ page }) => {
  const manifest = manifestDocument([
    ...siteManifest.entries,
    dirEntry('writing', 'writing'),
    fileEntry('.websh/ledger.json', 'ledger', { kind: 'data' }),
    fileEntry('writing/content-backed-homepage.md', 'content-backed homepage')
  ]);
  rawResponses.set('/content/manifest.json', JSON.stringify(manifest));
  rawResponses.set('/content/writing/content-backed-homepage.md', '# content-backed homepage');

  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/writing/content-backed-homepage`, { waitUntil: 'networkidle' });
  const sigchip = page.getByRole('button', { name: 'Signature of this page' });
  await expect(sigchip).toBeVisible({ timeout: 10000 });
  await sigchip.click();
  await expect(page.locator('body')).toContainText('/writing/content-backed-homepage');
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

test('theme selection applies globally and persists', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'kanagawa-wave');

  await page.getByRole('button', { name: /palette/i }).click();
  await page.getByRole('option', { name: /Black Ink/i }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'black-ink');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), themeStorageKey)).toBe('black-ink');

  await page.goto(`${baseUrl}/`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('A Homepage, Formalised', { timeout: 10000 });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'black-ink');

  await page.getByRole('button', { name: /palette/i }).click();
  await page.getByRole('option', { name: /Sepia Dark/i }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'sepia-dark');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), themeStorageKey)).toBe('sepia-dark');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('legacy shell hash canonicalizes to websh', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/shell/db`, { waitUntil: 'networkidle' });
  await expect.poll(() => new URL(page.url()).hash).toBe('#/websh/db');
  await expect(page.locator('body')).toContainText('~/websh/db', { timeout: 10000 });

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('draft changes survive reload through IndexedDB', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'login', 'Connected:');
  await runCommand(page, 'echo persisted > persist.md');
  await waitForDraftPath(page, '/persist.md');

  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls', 'persist.md');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('github token is represented by marker, not raw state file', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'sync auth set qa-token', 'sync auth set <redacted>');
  await expect(page.locator('body')).not.toContainText('qa-token');
  await page.keyboard.press('ArrowUp');
  await expect(page.locator('input[type="text"]')).not.toHaveValue(/qa-token/);
  await runCommand(page, 'ls /.websh/state/session', 'github_token_present');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls /.websh/state/session', 'github_token_present');
  await runCommand(page, 'sync auth clear');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls /.websh/state/session');
  await expect(page.locator('body')).not.toContainText('github_token_present');
  await runCommand(page, 'cat /.websh/state/session/github_token', 'No such file or directory');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('sync commit sends token and normalized GitHub file changes', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  const graphqlRequests = [];

  await page.route('https://api.github.com/graphql', async (route) => {
    const request = route.request();
    const body = JSON.parse(request.postData() || '{}');
    const input = body.variables.input;
    graphqlRequests.push({
      authorization: request.headers().authorization,
      input
    });

    const manifestAddition = input.fileChanges.additions.find((addition) => addition.path === 'content/manifest.json');
    const updatedManifest = Buffer.from(manifestAddition.contents, 'base64').toString('utf8');
    rawResponses.set('/content/manifest.json', updatedManifest);
    rawResponses.set('/content/commit-new.md', 'commit-new');
    rawResponses.delete('/content/docs/old.md');
    rawResponses.delete('/content/docs/deep/old.md');

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        data: {
          createCommitOnBranch: {
            commit: { oid: '2222222222222222222222222222222222222222' }
          }
        }
      })
    });
  });

  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await putMetadata(page, 'remote_head.~', expectedHead);
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });

  await runCommand(page, 'login', 'Connected:');
  await runCommand(page, 'sync auth set qa-token', 'sync auth set <redacted>');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'echo commit-new > commit-new.md');
  await runCommand(page, 'echo changed > docs/old.md');
  await runCommand(page, 'rm -r docs');
  await runCommand(page, 'sync commit qa commit', 'sync: committed 3 files');
  await runCommand(page, 'sync status', 'working tree clean');

  expect(graphqlRequests).toHaveLength(1);
  const [{ authorization, input }] = graphqlRequests;
  expect(authorization).toBe('bearer qa-token');
  expect(input.branch.repositoryNameWithOwner).toBe('0xwonj/websh');
  expect(input.branch.branchName).toBe('main');
  expect(input.message.headline).toBe('qa commit');
  expect(input.expectedHeadOid).toBe(expectedHead);
  const additions = input.fileChanges.additions.map((addition) => addition.path).sort();
  const deletions = input.fileChanges.deletions.map((deletion) => deletion.path).sort();
  expect(additions).toEqual(['content/commit-new.md', 'content/manifest.json']);
  expect(deletions).toEqual(['content/docs/deep/old.md', 'content/docs/old.md']);

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
