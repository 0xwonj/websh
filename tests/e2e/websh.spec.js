const { test, expect } = require('playwright/test');

const admin = '0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a';

const siteManifest = {
  files: [
    { path: 'index.html', title: 'Home', size: null, modified: null, tags: [], access: null },
    { path: '.websh/site.json', title: 'Site', size: null, modified: null, tags: [], access: null },
    { path: '.websh/index.json', title: 'Index', size: null, modified: null, tags: [], access: null },
    { path: '.websh/mounts/db.mount.json', title: 'DB mount', size: null, modified: null, tags: [], access: null }
  ],
  directories: [
    { path: '', title: 'Home', tags: [], description: null, icon: null, thumbnail: null },
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

const textResponses = new Map([
  ['/0xwonj/db/main/~/manifest.json', JSON.stringify(siteManifest)],
  ['/0xwonj/db/main/~/index.html', '<main><h1>Home OK</h1></main>'],
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

test.beforeEach(async ({ page }) => {
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
    const body = textResponses.get(url.pathname);
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
  await page.locator('input[type="text"]').fill(input);
  await page.keyboard.press('Enter');
  if (expectedText) {
    await expect(page.locator('body')).toContainText(expectedText, { timeout: 10000 });
  }
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
    await page.goto(`http://127.0.0.1:4173${hashPath}`, { waitUntil: 'networkidle' });
    await expect(page.locator('body')).toContainText(expectedText, { timeout: 10000 });
    expect(pageErrors).toEqual([]);
    expect(consoleErrors).toEqual([]);
  });
}

test('draft changes survive reload through IndexedDB', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto('http://127.0.0.1:4173/#/shell', { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'login', 'Connected:');
  await runCommand(page, 'echo persisted > persist.md');
  await page.waitForTimeout(800);

  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('input[type="text"]')).toBeVisible({ timeout: 10000 });
  await runCommand(page, 'ls', 'persist.md');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});

test('github token is represented by marker, not raw state file', async ({ page }) => {
  const { pageErrors, consoleErrors } = await collectBrowserErrors(page);
  await page.goto('http://127.0.0.1:4173/#/shell', { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await runCommand(page, 'sync auth set qa-token');
  await runCommand(page, 'ls /state/session', 'github_token_present');
  await runCommand(page, 'cat /state/session/github_token', 'No such file or directory');

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);
});
