const fs = require("node:fs");
const path = require("node:path");
const { root, env, run } = require("./tools.cjs");

function build(rootDir = root, invoke = run) {
  // Sync writes manifests and attestation state. Verify an unsigned source copy
  // with its own Cargo cache: two checkouts must never share compiled outputs.
  const stage = path.join(rootDir, "target", "verify", "source");
  const dist = path.join(rootDir, "target", "verify", "dist");
  fs.rmSync(stage, { recursive: true, force: true });
  fs.mkdirSync(stage, { recursive: true });
  for (const name of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Trunk.toml", "index.html", "_headers", "crates", "assets", "content", "vendor"]) {
    fs.cpSync(path.join(rootDir, name), path.join(stage, name), { recursive: true, preserveTimestamps: true, dereference: true });
  }
  // Sync retains matching signatures, so remove staged subjects before the hook
  // rebuilds them. Keep the header for its normal validation.
  const attestationsPath = path.join(stage, "assets", "crypto", "attestations.json");
  const attestations = JSON.parse(fs.readFileSync(attestationsPath, "utf8"));
  if (!Array.isArray(attestations.subjects)) throw new Error("Attestation subjects must be an array");
  attestations.subjects = [];
  fs.writeFileSync(attestationsPath, `${JSON.stringify(attestations, null, 2)}\n`);
  const buildEnv = { ...env, CARGO_TARGET_DIR: path.join(rootDir, "target", "verify-cargo") };
  delete buildEnv.NO_COLOR;
  invoke("trunk", ["build", "--release", "--locked", "--dist", dist], { cwd: stage, env: buildEnv });
  return dist;
}

module.exports = { build };

if (require.main === module) console.log(`Verification build: ${build()}`);
