const {test,expect,baseUrl,siteEntries,fileEntry,dirEntry,publishRoot,rootPath,home,rawOrigin} = require('./support/fixtures');
const {signManifest} = require('./support/signing.cjs');
const {deferred} = require('./support/browser');

async function proof(page, route) {
  await page.goto(`${baseUrl}/#${route}`, {waitUntil:'networkidle'});
  const chip = page.getByRole('button', {name:'Signature of this page'});
  await expect(chip.locator('[data-state="verified"]')).toBeVisible();
  await chip.click();
  const details = page.getByRole('tooltip');
  const message = await details.locator('[class*="sigRow"]').filter({hasText:/^message /}).innerText();
  return {message, details};
}

test('page messages bind their own views and a Now edit preserves a document proof', async ({page,responses}) => {
  const input = {entries:[...siteEntries,dirEntry('writing','writing'),fileEntry('writing/note.md','Note',{date:'2026-01-01'})],files:{'writing/note.md':'# Note'},publications:['writing/note.md']};
  publishRoot(responses,input);
  const first = await proof(page,'/');
  expect(first.message).toContain('SHA256(home @ /)');
  const ledger = await proof(page,'/ledger');
  expect(ledger.message).toContain('SHA256(ledger @ /ledger)');
  const writing = await proof(page,'/writing');
  expect(writing.message).toContain('SHA256(ledger @ /writing)');
  const note = await proof(page,'/writing/note');
  expect(note.message).toContain('SHA256(page @ /writing/note)');
  expect(new Set([first.message,ledger.message,writing.message,note.message]).size).toBe(4);
  publishRoot(responses,{...input,sequence:2,commit:'c'.repeat(40),projection:{...home,now:{items:[{date:'2026-01-02',text:'Changed Now'}]}}});
  await page.reload({waitUntil:'networkidle'});
  expect((await proof(page,'/')).message).not.toBe(first.message);
  expect((await proof(page,'/ledger')).message).toBe(ledger.message);
  expect((await proof(page,'/writing/note')).message).toBe(note.message);
});

test('root authentication never grants a missing, invalid or unread page a green SIG', async ({page,responses}) => {
  for (const mode of ['missing','invalid']) {
    const commit = (mode==='missing'?'a':'c').repeat(40);
    const manifest = publishRoot(responses,{sequence:mode==='missing'?1:2,commit});
    const subject = manifest.release.attestations.subjects.find(subject=>subject.route==='/');
    if (mode==='missing') subject.attestations=[];
    else subject.attestations[0].message_sha256='0x'+'0'.repeat(64);
    const body=JSON.stringify(manifest);
    responses.set(rootPath('manifest.json',commit),body);
    responses.set(rootPath('manifest.sig',commit),signManifest(body));
    await page.goto(`${baseUrl}/#/`,{waitUntil:'networkidle'});
    await page.reload({waitUntil:'networkidle'});
    await expect(page.locator('body')).toContainText('Fixture Now item');
    await expect(page.getByRole('button',{name:'Signature of this page'}).locator(`[data-state="${mode==='missing'?'unsigned':'invalid'}"]`)).toBeVisible();
  }
  publishRoot(responses,{sequence:3,commit:'d'.repeat(40),entries:[...siteEntries,dirEntry('writing','writing'),fileEntry('writing/note.md','Note')],files:{'writing/note.md':'# Note'},publications:['writing/note.md']});
  await page.reload({waitUntil:'networkidle'});
  const gate=deferred(); const started=deferred();
  await page.route(`${rawOrigin}${rootPath('writing/note.md','d'.repeat(40))}`,async route=>{started.resolve();await gate.promise;await route.fulfill({status:200,body:'# Fake'});});
  await page.goto(`${baseUrl}/#/writing/note`,{waitUntil:'domcontentloaded'});await started.promise;
  const chip=page.getByRole('button',{name:'Signature of this page'});
  await expect(chip.locator('[data-state="pending"]')).toBeVisible();
  gate.resolve();
  await expect(page.locator('[data-reader-body]')).toContainText(/integrity|hash/i);
  await expect(chip.locator('[data-state="invalid"]')).toBeVisible();
});


test('a grouped Markdown reader resolves its signed directory by actual file membership', async ({page,responses}) => {
  publishRoot(responses, {
    entries: [...siteEntries, dirEntry('writing','writing'), dirEntry('writing/group','Group'), fileEntry('writing/group/note.md','Grouped note')],
    files: {'writing/group/note.md':'# Grouped note'}, publications: ['writing/group'],
  });
  const note = await proof(page, '/writing/group/note');
  expect(note.message).toContain('SHA256(directory @ /writing/group)');
  await expect(page.locator('[data-reader-body]')).toContainText('Grouped note');
});
