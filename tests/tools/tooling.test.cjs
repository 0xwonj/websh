const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const { publicModules, forbiddenEdges } = require("../../scripts/architecture.cjs");
const { build } = require("../../scripts/build-check.cjs");
const { checkTrunkTools } = require("../../scripts/tools.cjs");
const { parseTestResult } = require("../wasm/result.cjs");

test("WASM result parsing rejects forged logs and failing terminal summaries", () => {
  const forged = "test result: ok. 99 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.01s";
  const failed = "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 filtered out; finished in 0.20s";
  assert.throws(() => parseTestResult(`${forged}\n${failed}\n`), /tests failed/);
  assert.throws(() => parseTestResult(`${forged}\nstill running\n`), /complete test summary/);
  assert.throws(() => parseTestResult(failed.replace("FAILED", "ok")), /tests failed/);
  assert.throws(() => parseTestResult(forged.replace("99 passed", "0 passed")), /did not execute/);
  assert.deepEqual(parseTestResult(`${forged}\n`), { passed: 99, ignored: 0, filtered: 0 });
});

test("facade discovery includes modules re-exported from the private engine", () => {
  const facades = publicModules("pub mod domain;\nmod engine;\npub use engine::{filesystem, runtime, shell};\n");
  assert.deepEqual(facades, ["domain", "filesystem", "runtime", "shell"]);
});

test("architecture enforces dependency directions without requiring unused edges", () => {
  const packages = [
    { name: "websh-core", dependencies: [{ name: "serde" }] },
    { name: "websh-site", dependencies: [] },
    { name: "websh-cli", dependencies: [{ name: "websh-core", rename: "model" }] },
    { name: "websh-web", dependencies: [] },
  ];
  assert.deepEqual(forbiddenEdges(packages), []);
  packages[0].dependencies.push({ name: "websh-site", rename: "policy" });
  packages[3].dependencies.push({ name: "websh-cli" });
  assert.deepEqual(forbiddenEdges(packages), ["websh-core->websh-site", "websh-web->websh-cli"]);
});

test("verification isolates generated sources and compiled outputs from the author checkout", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "websh-build-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const dir of ["crates", "assets/crypto", "content", "vendor"]) {
    fs.mkdirSync(path.join(root, dir), { recursive: true });
  }
  for (const file of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "Trunk.toml", "index.html", "_headers"]) {
    fs.writeFileSync(path.join(root, file), "fixture");
  }
  const authored = JSON.stringify({ scheme: "fixture", subjects: [{ signature: "retain" }] });
  const artifact = path.join("assets", "crypto", "attestations.json");
  fs.writeFileSync(path.join(root, artifact), authored);
  let invoked = false;
  const dist = build(root, (command, args, options) => {
    invoked = true;
    assert.equal(command, "trunk");
    assert.deepEqual(args, ["build", "--release", "--locked", "--dist", path.join(root, "target/verify/dist")]);
    assert.equal(options.cwd, path.join(root, "target/verify/source"));
    assert.equal(options.env.CARGO_TARGET_DIR, path.join(root, "target/verify-cargo"));
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(options.cwd, artifact))), { scheme: "fixture", subjects: [] });
  });
  assert.ok(invoked);
  assert.equal(dist, path.join(root, "target/verify/dist"));
  assert.equal(fs.readFileSync(path.join(root, artifact), "utf8"), authored);
});

test("Trunk's Binaryen pin must match the bootstrap requirement", () => {
  const { version } = require("../../scripts/tools.json")["wasm-opt"];
  checkTrunkTools(`[tools]\nwasm_opt = "version_${version}"\n[build]\ntarget = "index.html"\n`);
  assert.throws(() => checkTrunkTools('[tools]\nwasm_opt = "version_1"\n'), /must match/);
  assert.throws(() => checkTrunkTools(`[build]\nwasm_opt = "version_${version}"\n`), /must match/);
});
