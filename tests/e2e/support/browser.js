const { expect, appOrigin } = require('./fixtures');

function deferred() {
  let resolve;
  const promise = new Promise((innerResolve) => {
    resolve = innerResolve;
  });
  return { promise, resolve };
}

function collectNavigationNetwork(page) {
  const mainDocumentRequests = [];
  const wasmResponses = [];
  const sameOriginFailures = [];

  page.on('request', (request) => {
    if (request.resourceType() === 'document' && request.frame() === page.mainFrame()) {
      mainDocumentRequests.push(request.url());
    }
  });

  page.on('response', (response) => {
    const url = new URL(response.url());
    if (url.origin === appOrigin && url.pathname.endsWith('_bg.wasm')) {
      wasmResponses.push(response.url());
    }
    if (
      url.origin === appOrigin &&
      response.status() >= 400 &&
      !url.pathname.startsWith('/.well-known/trunk/')
    ) {
      sameOriginFailures.push(`${response.status()} ${url.pathname}`);
    }
  });

  return { mainDocumentRequests, wasmResponses, sameOriginFailures };
}

async function installIpfsBaseAlias(page, cid = 'fakecid') {
  await page.route(`**/ipfs/${cid}/**`, async (route) => {
    const url = new URL(route.request().url());
    const targetPath = `/${url.pathname.replace(new RegExp(`^/ipfs/${cid}/?`), '')}`;
    const response = await route.fetch({ url: `${appOrigin}${targetPath}${url.search}` });
    await route.fulfill({ response });
  });
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

async function cacheRecords(page) {
  return page.evaluate(async () => {
    if (!(await indexedDB.databases()).some((db) => db.name === 'websh-cache')) return [];
    return new Promise((resolve, reject) => {
      const request = indexedDB.open('websh-cache');
      request.onerror = () => reject(request.error);
      request.onsuccess = () => {
        const db = request.result;
        if (!db.objectStoreNames.contains('mount_snapshots')) { db.close(); resolve([]); return; }
        const tx = db.transaction('mount_snapshots', 'readonly');
        const read = tx.objectStore('mount_snapshots').getAll();
        tx.oncomplete = () => { db.close(); resolve(read.result); };
        tx.onerror = () => { db.close(); reject(tx.error); };
      };
    });
  });
}

module.exports = { deferred, collectNavigationNetwork, installIpfsBaseAlias, runCommand, cacheRecords };
