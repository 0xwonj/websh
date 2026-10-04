const { test, expect, baseUrl } = require('./support/fixtures');
const { runCommand } = require('./support/browser');

const themeStorageKey = 'user.THEME';

test('theme controls and shell preferences share persistent state', async ({ page }) => {
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

  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await runCommand(page, 'export THEME=sepia-dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'sepia-dark');
  await expect.poll(() => page.evaluate((key) => localStorage.getItem(key), themeStorageKey)).toBe('sepia-dark');
});

test('unsupported command input never reaches output, history, storage, or requests', async ({ page }) => {
  const requests = [];
  page.on('request', request => requests.push(request.url() + JSON.stringify(request.headers()) + (request.postData() || '')));
  await page.addInitScript(() => {
    localStorage.setItem('user.LANG', 'ko');
  });
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');
  await runCommand(page, 'unknown-command private-input-sentinel');
  await expect(page.locator('body')).toContainText('Unsupported command or syntax.');
  await expect(page.locator('body')).not.toContainText('private-input-sentinel');
  await page.keyboard.press('ArrowUp');
  await expect(page.locator('input[type="text"]')).not.toHaveValue(/private-input-sentinel/);
  expect(await page.evaluate(() => localStorage.getItem('user.LANG'))).toBe('ko');
  expect(await page.evaluate(() => JSON.stringify(localStorage))).not.toContain('private-input-sentinel');
  expect(requests.join(' ')).not.toMatch(/private-input-sentinel/);
});

test('history replay retains executed arguments across repeated references', async ({ page }) => {
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');
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
