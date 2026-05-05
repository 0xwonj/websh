const { spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');

function walk(dir, predicate, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(fullPath, predicate, out);
    } else if (predicate(fullPath)) {
      out.push(fullPath);
    }
  }
  return out;
}

function rel(file) {
  return path.relative(root, file).split(path.sep).join('/');
}

const moduleFiles = walk(
  path.join(root, 'crates', 'websh-web', 'src'),
  (file) => file.endsWith('.module.css'),
).map(rel).sort();

if (moduleFiles.length === 0) {
  console.error('lint:css: no CSS module files matched crates/websh-web/src/**/*.module.css');
  process.exit(1);
}

const assetFiles = [
  ...walk(path.join(root, 'assets', 'tokens'), (file) => file.endsWith('.css')),
  ...walk(path.join(root, 'assets', 'themes'), (file) => file.endsWith('.css')),
  path.join(root, 'assets', 'base.css'),
]
  .filter((file) => fs.existsSync(file))
  .map(rel)
  .sort();

const stylelintBin = path.join(
  root,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'stylelint.cmd' : 'stylelint',
);

if (!fs.existsSync(stylelintBin)) {
  console.error('lint:css: missing node_modules/.bin/stylelint; run npm install');
  process.exit(1);
}

const result = spawnSync(stylelintBin, [...moduleFiles, ...assetFiles], {
  cwd: root,
  stdio: 'inherit',
});

if (result.error) {
  console.error(`lint:css: failed to run stylelint: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
