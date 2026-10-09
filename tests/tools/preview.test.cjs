const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {once} = require('node:events');
const {serve} = require('../../scripts/serve-dist.cjs');

test('preview serves only frozen inventory and confined assets without modifying the build', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(),'websh-preview-'));
  fs.mkdirSync(path.join(root,'dist'));
  fs.writeFileSync(path.join(root,'dist/index.html'),'<head></head>');
  fs.writeFileSync(path.join(root,'private'),'private');
  fs.symlinkSync(path.join(root,'private'),path.join(root,'dist/escape'));
  const server = serve(path.join(root,'dist'),0,{responses:new Map([['/owner/repo/file',Buffer.from('public')]]),adapter:'<script>/* preview */</script>'});
  try {
    await once(server,'listening');
    const base = `http://127.0.0.1:${server.address().port}`;
    assert.match(await (await fetch(base)).text(),/preview/);
    assert.equal(fs.readFileSync(path.join(root,'dist/index.html'),'utf8'),'<head></head>');
    assert.equal(await (await fetch(`${base}/_preview/owner/repo/file`)).text(),'public');
    assert.equal((await fetch(`${base}/_preview/owner/repo/private`)).status,404);
    assert.equal((await fetch(`${base}/escape`)).status,403);
    assert.equal((await fetch(base,{method:'POST'})).status,405);
    assert.equal(await (await fetch(`${base}/_preview/owner/repo/file`,{method:'HEAD'})).text(),'');
  } finally {
    await new Promise(resolve=>server.close(resolve));
    fs.rmSync(root,{recursive:true,force:true});
  }
});
