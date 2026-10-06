const { test, expect, baseUrl, walletAddress } = require('./support/fixtures');
const { runCommand, cacheRecords } = require('./support/browser');

test.beforeEach(async ({ page }) => {
  await page.addInitScript(address => {
    window.ethereum = {
      request: async ({ method }) => method === 'eth_chainId' ? '0x1' : [address]
    };
  }, walletAddress);
});

test('wallet connection restores across reload and logout clears the session', async ({ page }) => {
  const mutations = [];
  page.on('request', request => { if (!['GET', 'HEAD', 'OPTIONS'].includes(request.method()) || /api\.github\.com|graphql/i.test(request.url()) || request.headers().authorization) mutations.push(request.url()); });
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await runCommand(page, 'login', 'Connected:');
  await runCommand(page, 'cat docs/old.md');
  await expect(page.locator('body')).toContainText('old');
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('Connected:');
  await runCommand(page, 'logout');
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');
  expect(await page.evaluate(() => localStorage.getItem('websh.wallet_session'))).toBeNull();
  expect(mutations).toEqual([]);
});

test('wallet account and chain events update identity without repartitioning public cache', async ({ page, errors }) => {
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
  errors.allowHttpError(/^https:\/\/api\.ensideas\.com\//, 503);
  await page.route('https://api.ensideas.com/**', route => route.fulfill({ status: 503, body: 'ENS unavailable' }));
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await runCommand(page, 'login', 'Connected:');
  await expect.poll(async () => (await cacheRecords(page)).length).toBe(2);
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

test('a declined wallet request leaves public content readable', async ({ page }) => {
  await page.addInitScript(() => {
    window.ethereum.request = async () => { throw new Error('user declined'); };
  });
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await runCommand(page, 'login');
  await expect(page.locator('body')).toContainText('user declined');
  await page.goto(`${baseUrl}/#/docs/old`, { waitUntil: 'networkidle' });
  await expect(page.locator('[data-reader-body]')).toContainText('old');
});
