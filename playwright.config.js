const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: './tests/e2e',
  timeout: 30000,
  forbidOnly: Boolean(process.env.CI),
  use: {
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    baseURL: process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173'
  },
  webServer: process.env.WEBSH_E2E_BASE_URL
    ? undefined
    : {
        command: 'node scripts/serve-dist.cjs',
        url: 'http://127.0.0.1:4173',
        reuseExistingServer: false,
        timeout: 10000
      }
});
