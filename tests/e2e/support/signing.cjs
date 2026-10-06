const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const root = path.resolve(__dirname, '../../..');
const fixture = path.join(root, 'tests/fixtures/pgp');
const home = path.join(root, 'target/verify/gnupg');
const { fingerprint } = require('../../fixtures/pgp/identity.json');
let ready = false;

function signManifest(body) {
  fs.mkdirSync(home, { recursive: true, mode: 0o700 });
  const args = ['--homedir', home, '--no-options', '--batch', '--yes', '--pinentry-mode', 'loopback', '--passphrase', ''];
  if (!ready) {
    execFileSync('gpg', [...args, '--import', path.join(fixture, 'private.asc')], { stdio: ['ignore', 'ignore', 'pipe'] });
    ready = true;
  }
  return execFileSync('gpg', [...args, '--local-user', `${fingerprint}!`, '--armor', '--detach-sign', '--output', '-'], { input: body, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });
}

module.exports = { signManifest };
