const fs = require("node:fs");
const path = require("node:path");

const outputs = [
  "dist",
  "dist-dev",
  "target/verify",
  "test-results",
  "playwright-report",
  "assets/bundle.css",
];
const caches = [
  "target/debug",
  "target/release",
  "target/wasm32-unknown-unknown",
  "target/wasm-bindgen",
  "target/wasm-opt",
  "target/vendor-tests",
  "target/doc",
  "target/package",
  "target/tmp",
  "target/.rustc_info.json",
  "target/.future-incompat-report.json",
];

// Only repository-owned paths are eligible. Never follow a redirected parent
// such as target -> another checkout, or touch tools, receipts, or author data.
function clean(root, scope, dryRun = false) {
  if (!["outputs", "cache"].includes(scope)) throw new Error(`Unknown cleanup scope: ${scope}`);
  root = fs.realpathSync(root);
  const paths = (scope === "cache" ? [...outputs, ...caches] : outputs).filter((relative) => {
    let current = root;
    for (const part of relative.split("/")) {
      current = path.join(current, part);
      const stat = fs.lstatSync(current, { throwIfNoEntry: false });
      if (!stat) return false;
      if (stat.isSymbolicLink()) throw new Error(`Refusing cleanup through symlink: ${current}`);
    }
    return true;
  });
  // Validate the entire plan before the first deletion.
  if (!dryRun) {
    for (const relative of paths) fs.rmSync(path.join(root, relative), { recursive: true, force: true });
  }
  return paths;
}

module.exports = { clean };

if (require.main === module) {
  try {
    const [scope, ...args] = process.argv.slice(2);
    if (args.length > 1 || args.some((arg) => arg !== "--dry-run")) {
      throw new Error("Usage: node scripts/clean.cjs outputs|cache [--dry-run]");
    }
    const dryRun = args.includes("--dry-run");
    const paths = clean(path.resolve(__dirname, ".."), scope, dryRun);
    for (const relative of paths) console.log(`${dryRun ? "Would remove" : "Removed"} ${relative}`);
    if (paths.length === 0) console.log("Nothing to clean.");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
