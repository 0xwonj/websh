const assert = require("node:assert/strict");
const { test } = require("node:test");
const { publicModules, workspaceEdges } = require("../scripts/architecture.cjs");
const { parseTestResult } = require("../scripts/wasm-result.cjs");

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
  const incompleteDocs = "websh_core::domain websh_core::filesystem websh_core::runtime";
  assert.deepEqual(facades.filter((name) => !incompleteDocs.includes(`websh_core::${name}`)), ["shell"]);
});

test("dependency edges use package identities, including renamed dependencies", () => {
  assert.deepEqual(workspaceEdges([
    { name: "core", dependencies: [] },
    { name: "cli", dependencies: [{ name: "core", rename: "model" }, { name: "serde" }, { name: "core" }] },
    { name: "web", dependencies: [{ name: "cli" }] },
  ]), ["cli->core", "web->cli"]);
});
