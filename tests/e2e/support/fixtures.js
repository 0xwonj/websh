const { test: base, expect } = require('@playwright/test');
const { createHash } = require('node:crypto');
const { signManifest } = require('./signing.cjs');
const ack = require('../../fixtures/ack.json');

const baseUrl = (process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173').replace(/\/+$/, '');
const appOrigin = new URL(baseUrl).origin;
const rawOrigin = 'https://raw.githubusercontent.com';
const rootRepo = '0xwonj/websh-content';
const rootCommit = 'a'.repeat(40);
const mountCommit = 'b'.repeat(40);
const rootPointer = `/${rootRepo}/main/current.json`;
const mountPointer = '/0xwonj/mount-db/main/current.json';
const rootPath = (path, commit = rootCommit) => `/${rootRepo}/${commit}/content/${path}`;
const mountPath = (path, commit = mountCommit, repo = '0xwonj/mount-db') => `/${repo}/${commit}/${path}`;
const walletAddress = '0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a';
const tinyPng = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=', 'base64');

function nodeMetadata(kind, { title, description = null, date = null, tags = [], size = null, childCount = null, bundle = null } = {}) {
  const authored = {};
  if (title != null) authored.title = title;
  if (description !== null) authored.description = description;
  if (date !== null) authored.date = date;
  if (tags.length) authored.tags = tags;
  const derived = {};
  if (size !== null) derived.size_bytes = size;
  if (childCount !== null) derived.child_count = childCount;
  return { kind, authored, derived, ...(bundle === null ? {} : {bundle}) };
}
function fileEntry(path, title, options = {}) {
  const ext = path.split('.').pop();
  return { path, metadata: nodeMetadata(options.kind || (['md','html'].includes(ext) ? 'page' : ext === 'pdf' ? 'document' : 'data'), {title, ...options}) };
}
function dirEntry(path, title, options = {}) { return { path, metadata: nodeMetadata('directory', {title, ...options}) }; }
function bundleEntry(path, title, options = {}) { return { path, metadata: nodeMetadata('bundle', {title, ...options}) }; }
const siteEntries = [dirEntry('', 'Home'), dirEntry('.site', 'Site support'), dirEntry('.site/errors', 'errors'), fileEntry('.site/errors/404.md', 'Not found'), dirEntry('docs', 'docs'), fileEntry('docs/old.md', 'Old')];
const rootFiles = {'docs/old.md': 'old', '.site/errors/404.md': '# Page not found\n\nThe requested page is not in this content snapshot.'};
const defaultMounts = [{backend:'github',trust:'unsigned',mount_at:'/db',repo:'0xwonj/mount-db',branch:'main',root:'',name:'db'}];
const home = {
  profile: {
    title:'wonjae.eth',tagline:'A Homepage, Formalised',name:'Fixture Author',affiliation:'Fixture University',email:'fixture@example.test',status:'accepting revisions',
    abstract_text:'A signed test homepage.',introduction:'Fixture introduction.',public_identity:'Fixture Author',private_identity:'test-only',research:['compilers'],tools:['Rust'],habits:['testing'],categories:['cs.PL'],keywords:['compilers'],links:[]
  }, now:{items:[{date:'2026-05-01',text:'Fixture Now item'}]}, ack
};
function indexed(entries, files) {
  const unique = new Map(entries.map(entry => [entry.path, structuredClone(entry)]));
  return [...unique.values()].map(entry => {
    if (!['directory','bundle'].includes(entry.metadata.kind)) {
      if (files[entry.path] === undefined) throw new Error(`Missing fixture body: ${entry.path}`);
      const bytes = Buffer.from(files[entry.path]);
      entry.metadata.derived.size_bytes = bytes.length;
      entry.metadata.derived.content_sha256 = createHash('sha256').update(bytes).digest('hex');
    }
    return entry;
  });
}
function publishRoot(responses, {entries=siteEntries,files={},projection=home,sequence=1,commit=rootCommit,publications=[],mounts=defaultMounts}={}) {
  files = {...rootFiles,...files};
  const manifest = {entries:indexed(entries,files),release:{purpose:'websh.content',site:'wonjae.eth',sequence,issued_at:Math.floor(Date.now()/1000),home:projection,mounts,publications}};
  const body = JSON.stringify(manifest);
  responses.set(rootPointer,JSON.stringify({commit}));
  responses.set(rootPath('manifest.json',commit),body);
  responses.set(rootPath('manifest.sig',commit),signManifest(body));
  for (const entry of manifest.entries) if (!['directory','bundle'].includes(entry.metadata.kind)) responses.set(rootPath(entry.path,commit),files[entry.path]);
  return manifest;
}
function publishMount(responses, {repo='0xwonj/mount-db',entries=[dirEntry('','DB'),fileEntry('fresh.md','Fresh')],files={'fresh.md':'# Fresh'},commit=mountCommit}={}) {
  const manifest={entries:indexed(entries,files)};
  responses.set(`/${repo}/main/current.json`,JSON.stringify({commit}));
  responses.set(mountPath('manifest.json',commit,repo),JSON.stringify(manifest));
  for(const [path,body] of Object.entries(files)) responses.set(mountPath(path,commit,repo),body);
  return manifest;
}
function freshResponses() { const responses=new Map(); publishRoot(responses);publishMount(responses);return responses; }
function contentPathEntries(path,title) {
  const parts=path.split('/'); const dirs=[];
  for(let i=0;i<parts.length-1;i++) dirs.push(dirEntry(parts.slice(0,i+1).join('/'),parts[i]));
  return [...dirs,fileEntry(path,title)];
}
function installContentPage(responses,path,title,body='# Fixture page') { publishRoot(responses,{entries:[...siteEntries,...contentPathEntries(path,title)],files:{[path]:body}}); }
function contentTypeForPath(path) {
  if(path.endsWith('.json')) return 'application/json';
  if(path.endsWith('.pdf')) return 'application/pdf';
  if(path.endsWith('.png')) return 'image/png';
  return 'text/plain';
}
async function installRoutes(page,responses) {
  await page.route(`${rawOrigin}/**`,async route=>{
    const url=new URL(route.request().url()); const body=responses.get(decodeURI(url.pathname));
    await route.fulfill({status:body===undefined?404:200,contentType:contentTypeForPath(url.pathname),body:body===undefined?`missing ${url.pathname}`:body});
  });
  await page.route('https://api.ensideas.com/**',route=>route.fulfill({status:200,contentType:'application/json',body:'{}'}));
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

module.exports = {test,expect,baseUrl,appOrigin,rawOrigin,walletAddress,tinyPng,fileEntry,dirEntry,bundleEntry,siteEntries,home,rootCommit,mountCommit,rootPointer,mountPointer,rootPath,mountPath,publishRoot,publishMount,installContentPage,installRoutes};
