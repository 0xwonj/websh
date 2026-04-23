const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  timeout: 30000,
  use: {
    baseURL: process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173'
  },
  webServer: process.env.WEBSH_E2E_BASE_URL
    ? undefined
    : {
        command: 'env -u NO_COLOR trunk serve --release --address 127.0.0.1 --port 4173',
        url: 'http://127.0.0.1:4173',
        reuseExistingServer: true,
        timeout: 120000
      }
});
