// Mempool — Phase 1 visual QA, updated in Phase 6 for URL-driven flows.
//
// Requires: trunk release build at WEBSH_E2E_BASE_URL, and at least 4 entries
// in 0xwonj/websh-mempool covering writing, projects, papers, talks
// categories. If the mempool repo is empty or missing, the first test fails
// by design — it's the canary for "mount is wired but no content arrived".

const { test, expect } = require('playwright/test');

const baseUrl = process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173';

test.describe('mempool', () => {
  test('renders above chain on /ledger with at least one entry', async ({ page }) => {
    await page.goto(`${baseUrl}/#/ledger`);
    const mempool = page.locator('section[aria-label="Mempool — pending entries"]');
    await expect(mempool).toBeVisible();
    const items = await mempool.locator('a').count();
    expect(items).toBeGreaterThan(0);
  });

  test('filter narrows mempool to category', async ({ page }) => {
    await page.goto(`${baseUrl}/#/writing`);
    const mempool = page.locator('section[aria-label="Mempool — pending entries"]');
    await expect(mempool).toBeVisible();
    const itemKinds = await mempool.locator('a [data-kind]').allTextContents();
    for (const kind of itemKinds) {
      expect(['writing']).toContain(kind);
    }
    // Header shows "X / Y pending"
    const headerText = await mempool.locator('span', { hasText: /pending/ }).innerText();
    expect(headerText).toMatch(/\d+ \/ \d+ pending/);
  });

  test('clicking a row navigates to the entry view URL', async ({ page }) => {
    await page.goto(`${baseUrl}/#/ledger`);
    const initialHash = await page.evaluate(() => window.location.hash);
    const firstRow = page
      .locator('section[aria-label="Mempool — pending entries"] a')
      .first();
    const href = await firstRow.getAttribute('href');
    expect(href).toMatch(/^\/#\/mempool\//);
    await firstRow.click();
    const afterClickHash = await page.evaluate(() => window.location.hash);
    expect(afterClickHash).not.toBe(initialHash);
    expect(afterClickHash).toMatch(/^#\/mempool\//);
    // Phase 6 dropped the modal preview entirely.
    await expect(page.locator('[aria-label="Close preview"]')).toHaveCount(0);
  });
});
