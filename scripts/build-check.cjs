const fs = require("node:fs");
const path = require("node:path");
const { root, env, run } = require("./tools.cjs");

function fixtureTrust(rootDir, stage) {
  const fixture = path.join(rootDir, "tests", "fixtures", "pgp");
  const { fingerprint } = JSON.parse(fs.readFileSync(path.join(fixture, "identity.json"), "utf8"));
  if (!/^[A-F0-9]{40}$/.test(fingerprint)) throw new Error("Invalid fixture fingerprint");
  fs.copyFileSync(path.join(fixture, "public.asc"), path.join(stage, "assets", "crypto", "site.asc"));
  const identity = path.join(stage, "crates", "websh-site", "src", "identity.rs");
  const source = fs.readFileSync(identity, "utf8");
  const pattern = /pub const EXPECTED_PGP_FINGERPRINT: &str = "[A-F0-9]{40}";/;
  if (!pattern.test(source)) throw new Error("Cannot pin explicit verification-build identity");
  fs.writeFileSync(identity, source.replace(pattern, `pub const EXPECTED_PGP_FINGERPRINT: &str = "${fingerprint}";`));
}

function build(rootDir = root, invoke = run) {
  // Build app sources without an authored content checkout. Keep an isolated
  // Cargo cache so staged sources cannot overwrite the working build outputs.
  const stage = path.join(rootDir, "target", "verify", "source");
  const dist = path.join(rootDir, "target", "verify", "dist");
  fs.rmSync(stage, { recursive: true, force: true });
  fs.mkdirSync(stage, { recursive: true });
  for (const name of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Trunk.toml", "index.html", "_headers", "crates", "assets", "vendor"]) {
    fs.cpSync(path.join(rootDir, name), path.join(stage, name), { recursive: true, preserveTimestamps: true, dereference: true });
  }
  fixtureTrust(rootDir, stage);
  const buildEnv = { ...env, CARGO_TARGET_DIR: path.join(rootDir, "target", "verify-cargo") };
  delete buildEnv.NO_COLOR;
  invoke("trunk", ["build", "--release", "--locked", "--dist", dist], { cwd: stage, env: buildEnv });
  return dist;
}

module.exports = { build };

if (require.main === module) console.log(`Verification build: ${build()}`);
