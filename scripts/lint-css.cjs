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

async function lint() {
  const { default: stylelint } = await import('stylelint');
  let failed = false;
  // Enumerate files ourselves and pass source text, never filename glob input.
  // Stylelint still applies the same repository configuration and overrides.
  for (const file of [...moduleFiles, ...assetFiles]) {
    const result = await stylelint.lint({
      code: fs.readFileSync(path.join(root, file), 'utf8'),
      codeFilename: path.join(root, file),
      configFile: path.join(root, '.stylelintrc.json'),
      cwd: root,
      formatter: 'string',
    });
    if (result.report) process.stdout.write(result.report);
    failed ||= result.errored;
  }
  if (failed) process.exitCode = 1;
}

lint().catch((error) => {
  console.error(`lint:css: ${error.message}`);
  process.exitCode = 1;
});
