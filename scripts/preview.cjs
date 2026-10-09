// Preview an actual immutable, signed content snapshot with production app trust.
// Only indexed public Git blobs are served. No directory or private-file endpoint.
const fs = require('node:fs');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const {createHash} = require('node:crypto');
const {serve} = require('./serve-dist.cjs');
const app = path.resolve(__dirname, '..');

function snapshot(root) {
  root = fs.realpathSync(root);
  execFileSync(path.join(app,'target/debug/websh-cli'), ['--root',root,'check','--require-signatures'], {cwd:app,stdio:'inherit'});
  const git = (...args) => execFileSync('git',args,{cwd:root,maxBuffer:70*1024*1024});
  const pointerBytes = git('show','HEAD:current.json');
  const {commit} = JSON.parse(pointerBytes);
  if (!/^[0-9a-f]{40}$/.test(commit)) throw new Error('Invalid snapshot commit');
  git('merge-base','--is-ancestor',commit,'HEAD');
  git('diff','--exit-code',commit,'--','content');
  const read = relative => git('show',`${commit}:content/${relative}`);
  const manifest = read('manifest.json');
  if (!manifest.equals(fs.readFileSync(path.join(root,'content/manifest.json')))) throw new Error('Local manifest differs from the selected snapshot');
  const signature = read('manifest.sig');
  if (!signature.equals(fs.readFileSync(path.join(root,'content/manifest.sig')))) throw new Error('Local signature differs from the selected snapshot');
  const prefix = '/0xwonj/websh-content/';
  const responses = new Map([[`${prefix}main/current.json`,pointerBytes],[`${prefix}${commit}/content/manifest.json`,manifest],[`${prefix}${commit}/content/manifest.sig`,signature]]);
  for (const entry of JSON.parse(manifest).entries) {
    const {derived} = entry.metadata;
    if (!derived?.content_sha256) continue;
    const bytes = read(entry.path);
    if (bytes.length !== derived.size_bytes || createHash('sha256').update(bytes).digest('hex') !== derived.content_sha256) throw new Error(`Snapshot integrity mismatch: ${entry.path}`);
    responses.set(`${prefix}${commit}/content/${entry.path}`,bytes);
  }
  const adapter = `<script>(()=>{const original=window.fetch.bind(window);window.fetch=(input,init)=>{const url=new URL(typeof input==='string'?input:input.url,location.href);if(url.origin==='https://raw.githubusercontent.com'&&url.pathname.startsWith('${prefix}')){const target=location.origin+'/_preview'+url.pathname;return original(input instanceof Request?new Request(target,input):target,init)}return original(input,init)}})();</script>`;
  return {responses,adapter};
}
module.exports = {snapshot};
if (require.main === module) {
  const root = process.argv[2];
  if (!root) throw new Error('Usage: node scripts/preview.cjs CONTENT_REPOSITORY [PORT]');
  serve(path.join(app,'dist-preview'),Number(process.argv[3] || 4186),snapshot(root));
}
