const {createHash}=require('node:crypto');
const {test,expect,baseUrl,rawOrigin,rootPointer,mountPointer,rootPath,home,publishRoot,rootCommit,installRoutes} = require('./support/fixtures');
const {deferred,cacheRecords}=require('./support/browser');
const rootRecord = records=>records.find(record=>record.descriptor.root==='/');

test('root renders before independent external discovery completes',async ({page})=>{
 const gate=deferred();
 await page.route(`${rawOrigin}${mountPointer}`,async route=>{await gate.promise;await route.fallback();});
 await page.goto(`${baseUrl}/#/`,{waitUntil:'domcontentloaded'});
 await expect(page.locator('body')).toContainText('Fixture Now item');
 gate.resolve();await page.goto(`${baseUrl}/#/db/fresh`,{waitUntil:'networkidle'});
 await expect(page.getByRole('heading',{name:'Fresh',exact:true}).first()).toBeVisible();
});

test('verified cached homepage survives offline discovery and a Now update retains unchanged bodies',async ({page,responses,errors})=>{
 await page.goto(`${baseUrl}/#/docs/old`,{waitUntil:'networkidle'});
 await expect(page.locator('[data-reader-body]')).toContainText('old');
 await expect.poll(async()=>Boolean(rootRecord(await cacheRecords(page)))).toBe(true);
 const oldCommit=rootCommit;const nextCommit='c'.repeat(40);
 let offline=true;errors.allowHttpError(`${rawOrigin}${rootPointer}`,503);
 await page.route(`${rawOrigin}${rootPointer}`,async route=>offline?route.fulfill({status:503,body:'offline'}):route.fallback());
 await page.goto(`${baseUrl}/#/`,{waitUntil:'networkidle'});await page.reload({waitUntil:'networkidle'});
 await expect(page.locator('body')).toContainText('Fixture Now item');
 await expect(page.locator('[data-mount-status]')).toContainText('Refresh failed');
 publishRoot(responses,{commit:nextCommit,sequence:2,projection:{...home,now:{items:[{date:'2026-05-02',text:'Updated Now without app build'}]}}});
 const bodies=[];page.on('request',request=>{if(request.url().endsWith('/docs/old.md'))bodies.push(request.url());});
 offline=false;
 await page.getByRole('button',{name:'Refresh listing'}).click();
 await expect(page.locator('body')).toContainText('Updated Now without app build');
 await expect.poll(async()=>rootRecord(await cacheRecords(page))?.source.commit).toBe(nextCommit);
 await page.goto(`${baseUrl}/#/docs/old`,{waitUntil:'networkidle'});
 await expect(page.locator('[data-reader-body]')).toContainText('old');
 expect(bodies).toEqual([]);
 expect(new URL(page.url()).pathname).toBe('/');
 expect(oldCommit).not.toBe(nextCommit);
});

test('unavailable IndexedDB does not block authenticated root or external reading',async ({page})=>{
 await page.addInitScript(()=>{IDBFactory.prototype.open=()=>{throw new DOMException('storage denied','SecurityError');};});
 await page.goto(`${baseUrl}/#/`,{waitUntil:'networkidle'});
 await expect(page.locator('body')).toContainText('Fixture Now item');
 await page.goto(`${baseUrl}/#/db/fresh`,{waitUntil:'networkidle'});
 await expect(page.getByRole('heading',{name:'Fresh',exact:true}).first()).toBeVisible();
});

test('external refresh failure retains a saved listing and retries independently',async ({page,errors})=>{
 await page.goto(`${baseUrl}/#/db/fresh`,{waitUntil:'networkidle'});
 await expect.poll(async()=>(await cacheRecords(page)).some(record=>record.descriptor.root==='/db')).toBe(true);
 let offline=true;errors.allowHttpError(`${rawOrigin}${mountPointer}`,503);
 await page.route(`${rawOrigin}${mountPointer}`,route=>offline?route.fulfill({status:503,body:'offline'}):route.fallback());
 await page.reload({waitUntil:'networkidle'});
 await expect(page.getByRole('heading',{name:'Fresh',exact:true}).first()).toBeVisible();
 await expect(page.locator('[data-mount-status]')).toContainText('Refresh failed');
 offline=false;await page.getByRole('button',{name:'Refresh listing'}).click();
 await expect(page.locator('[data-mount-status]')).toHaveCount(0);
});

test('historical snapshots validate exact identity without rolling back live cache',async ({page,responses})=>{
 const oldBody=responses.get(rootPath('manifest.json'));
 const oldId=createHash('sha256').update(oldBody).digest('hex');
 const next='d'.repeat(40);
 publishRoot(responses,{commit:next,sequence:2,projection:{...home,now:{items:[{date:'2026-05-02',text:'Latest live content'}]}}});
 await page.goto(`${baseUrl}/#/`,{waitUntil:'networkidle'});
 await expect(page.locator('body')).toContainText('Latest live content');
 await expect.poll(async()=>rootRecord(await cacheRecords(page))?.source.commit).toBe(next);
 await page.goto(`${baseUrl}/?content=${rootCommit}&release=${oldId}#/`,{waitUntil:'networkidle'});
 await expect(page.locator('body')).toContainText('Fixture Now item');
 await expect(page.locator('a[href*="?content="]')).toHaveCount(0);
 await page.getByRole('button',{name:'Signature of this page'}).click();
 await expect(page.getByRole('tooltip').getByRole('link',{name:`${baseUrl}/?content=${rootCommit}&release=${oldId}#/`,exact:true})).toHaveAttribute('href',`${baseUrl}/?content=${rootCommit}&release=${oldId}#/`);
 expect(rootRecord(await cacheRecords(page)).source.commit).toBe(next);
 await page.goto(`${baseUrl}/?content=${rootCommit}&release=${'f'.repeat(64)}#/`,{waitUntil:'networkidle'});
 await expect(page.locator('body')).not.toContainText('Fixture Author');
 await expect(page.locator('[data-mount-status]')).toContainText(/failed|mismatch|unavailable/i);
});

test('a shared root watermark rejects late older candidates and same-sequence replacements', async ({page, context, responses}) => {
  const signatureRequested = deferred();
  const finishOldSignature = deferred();
  const oldSignature = responses.get(rootPath('manifest.sig'));
  await page.route(`${rawOrigin}${rootPath('manifest.sig')}`, async route => {
    signatureRequested.resolve();
    await finishOldSignature.promise;
    await route.fulfill({body: oldSignature});
  });
  await page.goto(`${baseUrl}/#/`, {waitUntil: 'domcontentloaded'});
  await signatureRequested.promise;

  const nextCommit = 'e'.repeat(40);
  publishRoot(responses, {
    commit: nextCommit,
    sequence: 2,
    projection: {...home, now: {items: [{date: '2026-05-02', text: 'Accepted newer release'}]}}
  });
  const newer = await context.newPage();
  await installRoutes(newer, responses);
  await newer.goto(`${baseUrl}/#/`, {waitUntil: 'networkidle'});
  await expect(newer.locator('body')).toContainText('Accepted newer release');
  await expect.poll(async () => rootRecord(await cacheRecords(newer))?.source.commit).toBe(nextCommit);

  finishOldSignature.resolve();
  await expect(page.locator('[data-mount-status]')).toContainText(/failed|older|rollback/i);
  await expect(page.locator('body')).not.toContainText('Fixture Now item');

  publishRoot(responses, {
    commit: 'f'.repeat(40),
    sequence: 2,
    projection: {...home, now: {items: [{date: '2026-05-03', text: 'Conflicting same-sequence release'}]}}
  });
  await newer.reload({waitUntil: 'networkidle'});
  await expect(newer.locator('body')).toContainText('Accepted newer release');
  await expect(newer.locator('body')).not.toContainText('Conflicting same-sequence release');
  await expect(newer.locator('[data-mount-status]')).toContainText(/failed|conflict|sequence/i);
  expect(rootRecord(await cacheRecords(newer)).source.commit).toBe(nextCommit);
  await newer.close();
});
