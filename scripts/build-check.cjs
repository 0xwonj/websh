const fs = require("node:fs");
const path = require("node:path");
const { root, env, run } = require("./tools.cjs");

// Sync writes manifests and attestation state. Run it in an isolated
// source copy so verification never edits the author's content or signatures.
const stage = path.join(root, "target", "verify", "source");
const dist = path.join(root, "target", "verify", "dist");
fs.rmSync(stage, { recursive: true, force: true });
fs.mkdirSync(stage, { recursive: true });
for (const name of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Trunk.toml", "index.html", "_headers", "crates", "assets", "content", "vendor"]) {
  fs.cpSync(path.join(root, name), path.join(stage, name), { recursive: true, preserveTimestamps: true, dereference: true });
}
// Sync retains matching signatures, so remove staged subjects before the hook
// rebuilds them. Keep the header for its normal validation.
const attestationsPath = path.join(stage, "assets", "crypto", "attestations.json");
const attestations = JSON.parse(fs.readFileSync(attestationsPath, "utf8"));
if (!Array.isArray(attestations.subjects)) throw new Error("Attestation subjects must be an array");
attestations.subjects = [];
fs.writeFileSync(attestationsPath, `${JSON.stringify(attestations, null, 2)}\n`);
const buildEnv = { ...env, CARGO_TARGET_DIR: path.join(root, "target") };
delete buildEnv.NO_COLOR;
run("trunk", ["build", "--release", "--locked", "--dist", dist], { cwd: stage, env: buildEnv });
console.log(`Verification build: ${dist}`);
