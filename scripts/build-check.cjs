const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { root, env, run } = require("./tools.cjs");

// Release hooks write manifests and attestation state. Run them in an isolated
// source copy so verification never edits the author's content or signatures.
const stage = path.join(root, "target", "verify", "source");
const dist = path.join(root, "target", "verify", "dist");
fs.rmSync(stage, { recursive: true, force: true });
fs.mkdirSync(stage, { recursive: true });
for (const name of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Trunk.toml", "index.html", "_headers", "crates", "assets", "content", "vendor"]) {
  fs.cpSync(path.join(root, name), path.join(stage, name), { recursive: true, preserveTimestamps: true, dereference: true });
}
const buildEnv = { ...env, CARGO_TARGET_DIR: path.join(root, "target"), WEBSH_NO_SIGN: "1" };
// Read the original history with paths relative to the staged content tree.
// No Git writes run in release hooks; disable optional index refreshes as well.
buildEnv.GIT_DIR = execFileSync("git", ["rev-parse", "--absolute-git-dir"], { cwd: root, encoding: "utf8" }).trim();
buildEnv.GIT_WORK_TREE = stage;
buildEnv.GIT_OPTIONAL_LOCKS = "0";
delete buildEnv.NO_COLOR;
run("trunk", ["build", "--release", "--locked", "--dist", dist], { cwd: stage, env: buildEnv });
console.log(`Verification build: ${dist}`);
