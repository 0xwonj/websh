const { test, expect } = require('playwright/test');

const baseUrl = process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173';
const admin = '0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a';
const expectedHead = '1111111111111111111111111111111111111111';

const siteManifest = {
  files: [
    { path: 'index.html', title: 'Home', size: null, modified: null, tags: [], access: null },
    { path: 'docs/old.md', title: 'Old', size: null, modified: null, tags: [], access: null },
    { path: 'docs/deep/old.md', title: 'Deep Old', size: null, modified: null, tags: [], access: null },
    { path: '.websh/site.json', title: 'Site', size: null, modified: null, tags: [], access: null },
    { path: '.websh/index.json', title: 'Index', size: null, modified: null, tags: [], access: null },
    { path: '.websh/mounts/db.mount.json', title: 'DB mount', size: null, modified: null, tags: [], access: null }
  ],
  directories: [
    { path: '', title: 'Home', tags: [], description: null, icon: null, thumbnail: null },
    { path: 'docs', title: 'docs', tags: [], description: null, icon: null, thumbnail: null },
    { path: 'docs/deep', title: 'deep', tags: [], description: null, icon: null, thumbnail: null },
    { path: '.websh', title: '.websh', tags: [], description: null, icon: null, thumbnail: null },
    { path: '.websh/mounts', title: 'mounts', tags: [], description: null, icon: null, thumbnail: null }
  ]
};

const dbManifest = {
  files: [
    { path: 'fresh.md', title: 'Fresh', size: null, modified: null, tags: [], access: null }
  ],
  directories: [
    { path: '', title: 'DB', tags: [], description: null, icon: null, thumbnail: null }
  ]
};

let rawResponses;

function freshRawResponses() {
  return new Map([
    ['/0xwonj/db/main/~/manifest.json', JSON.stringify(siteManifest)],
    ['/0xwonj/db/main/~/index.html', '<main><h1>Home OK</h1></main>'],
    ['/0xwonj/db/main/~/docs/old.md', 'old'],
    ['/0xwonj/db/main/~/docs/deep/old.md', 'deep old'],
    ['/0xwonj/db/main/~/.websh/site.json', '{}'],
    ['/0xwonj/db/main/~/.websh/index.json', JSON.stringify({
      routes: [
        { route: '/', node_path: '/site/index.html', kind: 'page', renderer: 'html_page' }
      ]
    })],
    ['/0xwonj/db/main/~/.websh/mounts/db.mount.json', JSON.stringify({
      backend: 'github',
      mount_at: '/mnt/db',
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

  await page.route('https://raw.githubusercontent.com/**', async (route) => {
    const url = new URL(route.request().url());
    const body = rawResponses.get(url.pathname);
    if (body === undefined) {
      await route.fulfill({ status: 404, contentType: 'text/plain', body: `missing ${url.pathname}` });
      return;
    }
    const contentType = url.pathname.endsWith('.json') ? 'application/json' : 'text/plain';
    await route.fulfill({ status: 200, contentType, body });
  });
});

async function collectBrowserErrors(page) {
  const pageErrors = [];
  const consoleErrors = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
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
  ['/#/', 'Home OK'],
  ['/#/shell', 'guest@wonjae.eth:~'],
  ['/#/fs', 'Location:/'],
  ['/#/fs/site', 'Location:~'],
  ['/#/fs/state/session', 'Location:/state/session'],
  ['/#/fs/mnt/db', 'Location:/mnt/db']
];

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

test('draft changes survive reload through IndexedDB', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/shell`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'login', 'Connected:');
  await runCommand(page, 'echo persisted > persist.md');
  await waitForDraftPath(page, '/site/persist.md');

  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls', 'persist.md');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('github token is represented by marker, not raw state file', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto(`${baseUrl}/#/shell`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'sync auth set qa-token', 'sync auth set <redacted>');
  await expect(page.locator('body')).not.toContainText('qa-token');
  await page.keyboard.press('ArrowUp');
  await expect(page.locator('input[type="text"]')).not.toHaveValue(/qa-token/);
  await runCommand(page, 'ls /state/session', 'github_token_present');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls /state/session', 'github_token_present');
  await runCommand(page, 'sync auth clear');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls /state/session');
  await expect(page.locator('body')).not.toContainText('github_token_present');
  await runCommand(page, 'cat /state/session/github_token', 'No such file or directory');

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

    const manifestAddition = input.fileChanges.additions.find((addition) => addition.path === '~/manifest.json');
    const updatedManifest = Buffer.from(manifestAddition.contents, 'base64').toString('utf8');
    rawResponses.set('/0xwonj/db/main/~/manifest.json', updatedManifest);
    rawResponses.set('/0xwonj/db/main/~/commit-new.md', 'commit-new');
    rawResponses.delete('/0xwonj/db/main/~/docs/old.md');
    rawResponses.delete('/0xwonj/db/main/~/docs/deep/old.md');

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

  await page.goto(`${baseUrl}/#/shell`, { waitUntil: 'networkidle' });
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
  expect(input.branch.repositoryNameWithOwner).toBe('0xwonj/db');
  expect(input.branch.branchName).toBe('main');
  expect(input.message.headline).toBe('qa commit');
  expect(input.expectedHeadOid).toBe(expectedHead);
  const additions = input.fileChanges.additions.map((addition) => addition.path).sort();
  const deletions = input.fileChanges.deletions.map((deletion) => deletion.path).sort();
  expect(additions).toEqual(['~/commit-new.md', '~/manifest.json']);
  expect(deletions).toEqual(['~/docs/deep/old.md', '~/docs/old.md']);

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
