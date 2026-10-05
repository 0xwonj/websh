const { test: base, expect } = require('@playwright/test');
const { emptyLedger } = require('./ledger');

const baseUrl = (process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173').replace(/\/+$/, '');
const appOrigin = new URL(baseUrl).origin;
const walletAddress = '0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a';
const tinyPng = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=',
  'base64'
);

function nodeMetadata(kind, { title, description = null, date = null, tags = [], size = null, childCount = null, bundle = null } = {}) {
  const authored = {};
  if (title !== undefined && title !== null) authored.title = title;
  if (description !== null) authored.description = description;
  if (date !== null) authored.date = date;
  if (tags.length > 0) authored.tags = tags;

  const derived = {};
  if (size !== null) derived.size_bytes = size;
  if (childCount !== null) derived.child_count = childCount;

  const metadata = {
    kind,
    authored,
    derived
  };
  if (bundle !== null) metadata.bundle = bundle;
  return metadata;
}

function fileEntry(path, title, options = {}) {
  const ext = path.split('.').pop();
  const kind = options.kind || (ext === 'md' || ext === 'html' ? 'page' : ext === 'pdf' ? 'document' : 'data');
  return {
    path,
    metadata: nodeMetadata(kind, {
      title,
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

function bundleEntry(path, title, options = {}) {
  return {
    path,
    metadata: nodeMetadata('bundle', { title, ...options })
  };
}

function manifestDocument(entries) {
  return { entries };
}

const siteEntries = [
  dirEntry('', 'Home'),
  dirEntry('.site', 'Site support', {
    description: 'Runtime support and trust metadata for the site.',
    tags: ['runtime', 'trust'],
    childCount: 3
  }),
  dirEntry('.websh', '.websh'),
  dirEntry('.websh/mounts', 'mounts'),
  dirEntry('docs', 'docs'),
  fileEntry('.websh/ledger.json', 'Ledger', { kind: 'data' }),
  fileEntry('.websh/mounts/db.mount.json', 'DB mount', { kind: 'data' }),
  fileEntry('docs/old.md', 'Old')
];

const siteManifest = manifestDocument(siteEntries);

const dbManifest = manifestDocument([
  dirEntry('', 'DB'),
  fileEntry('fresh.md', 'Fresh')
]);

function fixturePathname(url) {
  return url.pathname.replace(/^\/ipfs\/[^/]+(?=\/)/, '');
}

function contentTypeForPath(pathname) {
  if (pathname.endsWith('.json')) return 'application/json';
  if (pathname.endsWith('.pdf')) return 'application/pdf';
  if (pathname.endsWith('.png')) return 'image/png';
  if (pathname.endsWith('.svg')) return 'image/svg+xml';
  return 'text/plain';
}

function freshResponses() {
  return new Map([
    ['/content/manifest.json', JSON.stringify(siteManifest)],
    ['/content/docs/old.md', 'old'],
    ['/content/.websh/ledger.json', JSON.stringify(emptyLedger())],
    ['/content/.websh/mounts/db.mount.json', JSON.stringify({
      backend: 'github',
      mount_at: '/db',
      repo: '0xwonj/mount-db',
      branch: 'main',
      root: '',
      name: 'db',
    })],
    ['/0xwonj/mount-db/main/manifest.json', JSON.stringify(dbManifest)],
    ['/0xwonj/mount-db/main/fresh.md', '# Fresh']
  ]);
}

function contentPathEntries(path, title) {
  const parts = path.split('/').filter(Boolean);
  const dirs = [];
  for (let idx = 0; idx < parts.length - 1; idx += 1) {
    const dirPath = parts.slice(0, idx + 1).join('/');
    dirs.push(dirEntry(dirPath, parts[idx]));
  }
  return [...dirs, fileEntry(path, title)];
}

function installContentPage(responses, path, title, body = '# Fixture page') {
  responses.set(
    '/content/manifest.json',
    JSON.stringify(manifestDocument([
      ...siteEntries,
      ...contentPathEntries(path, title)
    ]))
  );
  responses.set(`/content/${path}`, body);
}

async function installRoutes(page, responses) {
  const serve = async route => {
    const url = new URL(route.request().url());
    const body = responses.get(fixturePathname(url));
    await route.fulfill({
      status: body === undefined ? 404 : 200,
      contentType: contentTypeForPath(url.pathname),
      body: body === undefined ? `missing ${url.pathname}` : body
    });
  };
  await page.route('**/content/**', serve);
  await page.route('https://raw.githubusercontent.com/**', serve);
  await page.route('https://api.ensideas.com/**', route =>
    route.fulfill({ status: 200, contentType: 'application/json', body: '{}' }));
}

// Playwright creates this map and these listeners separately for every test.
const test = base.extend({
  responses: async ({}, use) => use(freshResponses()),
  page: async ({ page, responses }, use) => {
    await installRoutes(page, responses);
    await use(page);
  },
  errors: [async ({ page, context }, use) => {
    const pageErrors = [];
    const consoleErrors = [];
    const allowedHttpErrors = [];
    const watch = tab => {
      tab.on('pageerror', error => pageErrors.push(error.stack || error.message));
      tab.on('console', message => {
        if (message.type() === 'error') {
          consoleErrors.push({ text: message.text(), url: message.location().url });
        }
      });
    };
    watch(page);
    context.on('page', watch);
    await use({
      allowHttpError: (url, status) => allowedHttpErrors.push({
        url: url instanceof RegExp ? url : new URL(url, baseUrl).href, status
      })
    });
    expect(pageErrors, 'uncaught browser errors').toEqual([]);
    expect(consoleErrors.filter(message => !allowedHttpErrors.some(({ url, status }) =>
      (url instanceof RegExp ? url.test(message.url) : url === message.url) &&
      message.text.includes(`server responded with a status of ${status}`)
    )), 'unexpected browser console errors').toEqual([]);
  }, { auto: true }]
});

module.exports = {
  test, expect, baseUrl, appOrigin, walletAddress, tinyPng,
  fileEntry, dirEntry, bundleEntry, manifestDocument, siteEntries, siteManifest,
  dbManifest, installContentPage, installRoutes
};
