const {test,expect,baseUrl,siteEntries,fileEntry,dirEntry,publishRoot,publishMount,rootPath} = require('./support/fixtures');

test('publication catalog filters the signed entry set',async ({page,responses})=>{
 publishRoot(responses,{entries:[...siteEntries,dirEntry('writing','writing'),dirEntry('projects','projects'),fileEntry('writing/note.md','A note',{date:'2026-04-20'}),fileEntry('projects/app.md','An app',{date:'2026-04-22'})],files:{'writing/note.md':'# A note','projects/app.md':'# An app'},publications:['writing/note.md','projects/app.md']});
 await page.goto(`${baseUrl}/#/writing`,{waitUntil:'networkidle'});
 await expect(page.getByRole('link',{name:/^writing 1$/})).toHaveAttribute('aria-current','page');
 await expect(page.locator('article')).toHaveCount(1);
 await expect(page.locator('article')).toContainText('A note');
 await page.goto(`${baseUrl}/#/ledger`,{waitUntil:'networkidle'});
 await expect(page.getByRole('link',{name:/^all 2$/})).toHaveAttribute('aria-current','page');
 await expect(page.locator('article').first()).toContainText('An app');
 await expect(page.getByRole('region',{name:'Ledger metadata'})).toContainText('verified snapshot');
 await expect(page.locator('article')).toHaveCount(2);
 await page.goto(`${baseUrl}/#/misc`,{waitUntil:'networkidle'});
 await expect(page.locator('body')).toContainText('no publications match this filter');
});

test('independent mempool uses its own snapshot and never inherits the root signature',async ({page,responses})=>{
 const mounts=[{backend:'github',trust:'unsigned',mount_at:'/mempool',repo:'0xwonj/websh-mempool',branch:'main',root:'',name:'mempool'}];
 publishRoot(responses,{mounts});
 publishMount(responses,{repo:'0xwonj/websh-mempool',entries:[dirEntry('','Mempool'),...['writing','projects'].map(category=>({...fileEntry(`${category}/fixture.md`,`${category} entry`),mempool:{status:'review',priority:'high',category}}))],files:{'writing/fixture.md':'# Pending writing\n\nFixture body.','projects/fixture.md':'# Pending project'}});
 const signedRoot=responses.get(rootPath('manifest.json'));
 await page.goto(`${baseUrl}/#/writing`,{waitUntil:'networkidle'});
 const mempool=page.getByRole('region',{name:'Mempool — pending blocks'});
 await mempool.getByRole('button',{name:/mempool/i}).click();
 await expect(mempool.locator('a [data-kind]')).toHaveText(['writing']);
 await mempool.getByRole('link',{name:/writing entry/}).click();
 await expect(page.locator('[data-reader-body]')).toContainText('Fixture body.');
 const footer=page.getByRole('button',{name:'Content release signature'});
 await expect(footer).toContainText('unsigned');
 await footer.click();await expect(page.locator('body')).toContainText('not authenticated by the root signature');
 expect(responses.get(rootPath('manifest.json'))).toBe(signedRoot);
});
