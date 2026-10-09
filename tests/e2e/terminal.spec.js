const { test, expect, baseUrl, rawOrigin, rootPath } = require('./support/fixtures');
const { deferred, runCommand } = require('./support/browser');

const themeStorageKey = 'user.THEME';

test('whoami loads the ASCII profile, respects pipes and does not refill cleared output', async ({ page, responses }) => {
  const profilePath = rootPath('.site/profile.txt');
  const gate = deferred();
  let profileRequests = 0;
  await page.route(`${rawOrigin}${profilePath}`, async route => {
    profileRequests += 1;
    await gate.promise;
    await route.fallback();
  });
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~');

  await runCommand(page, 'WHOAMI');
  await expect.poll(() => profileRequests).toBe(1);
  await runCommand(page, 'clear');
  const finished = page.waitForEvent('requestfinished', request => request.url() === `${rawOrigin}${profilePath}`);
  gate.resolve();
  await finished;
  await page.waitForLoadState('networkidle');
  await expect(page.locator('body')).not.toContainText('Fixture Author');

  await runCommand(page, 'whoami');
  const ascii = page.locator('pre').filter({ hasText: 'Fixture Author' });
  await expect(ascii).toHaveCount(1);
  expect(await ascii.textContent()).toBe(responses.get(profilePath));
  await runCommand(page, 'clear');
  await runCommand(page, 'WhoAmI | grep Fixture | wc');
  await expect(page.getByText('1', { exact: true })).toBeVisible();
  await expect(ascii).toHaveCount(0);
  expect(profileRequests).toBe(1);
});

test('CSS validates the saved palette before the app starts', async ({ page }) => {
  // Preserve the real stylesheets and prepaint scripts, but do not start Wasm.
  await page.route(`${baseUrl}/**`, async route => {
    if (route.request().resourceType() !== 'document') return route.fallback();
    const response = await route.fetch();
    const body = (await response.text()).replace(/<script type="module">[\s\S]*?<\/script>/g, '');
    await route.fulfill({ response, body });
  });
  await page.addInitScript(key => {
    const params = new URLSearchParams(location.search);
    if (params.has('deny-storage')) {
      Object.defineProperty(window, 'localStorage', { get() { throw new DOMException('unavailable', 'SecurityError'); } });
    } else {
      localStorage.setItem(key, params.get('theme'));
    }
  }, themeStorageKey);
  for (const [query, expected] of [['theme=%20BLACK-INK%20', 'black-ink'], ['theme=unknown', 'kanagawa-wave'], ['deny-storage', 'kanagawa-wave']]) {
    await page.goto(`${baseUrl}/?${query}`, { waitUntil: 'domcontentloaded' });
    await expect(page.locator('html')).toHaveAttribute('data-theme', expected);
    const palette = await page.evaluate(() => ({
      background: getComputedStyle(document.documentElement).getPropertyValue('--bg-primary').trim(),
      meta: document.querySelector('meta[name="theme-color"]').content,
      started: Boolean(window.wasmBindings)
    }));
    expect(palette.background).not.toBe('');
    expect(palette.meta).toBe(palette.background);
    expect(palette.started).toBe(false);
  }
  const manifest = await (await page.request.get(`${baseUrl}/assets/manifest.json`)).json();
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute('content', manifest.theme_color);
  expect(manifest.background_color).toBe(manifest.theme_color);
});

test('theme controls and shell preferences share persistent state', async ({ page }) => {
  await page.goto(`${baseUrl}/#/websh`, { waitUntil: 'networkidle' });
  await expect(page.locator('body')).toContainText('guest@wonjae.eth:~', { timeout: 10000 });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'kanagawa-wave');

  await page.getByRole('button', { name: /palette/i }).click();
  const swatch = page.getByRole('button', { name: /Black Ink/i }).locator('[data-theme]');
  const colors = await swatch.evaluate(element => {
    const style = getComputedStyle(element);
    return { background: style.getPropertyValue('--bg-primary').trim(), accent: style.getPropertyValue('--accent').trim() };
  });
  expect(colors.background).not.toBe(colors.accent);
  await page.getByRole('button', { name: /Black Ink/i }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'black-ink');
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute('content', colors.background);
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
