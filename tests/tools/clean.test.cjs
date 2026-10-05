const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const { clean } = require("../../scripts/clean.cjs");

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "websh-clean-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const write = (relative) => {
    const file = path.join(root, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, relative);
  };
  return { root, write, exists: (relative) => fs.existsSync(path.join(root, relative)) };
}

test("cleanup scopes and dry-run preserve tools, receipts, and author data", (t) => {
  const { root, write, exists } = fixture(t);
  const generated = ["dist/index.html", "dist-preview/index.html", "target/verify/source/index.html", "assets/bundle.css"];
  const cached = [
    "target/debug/app",
    "target/wasm32-unknown-unknown/release/app.wasm",
    "target/wasm-bindgen/release/app.js",
    "target/wasm-opt/release/app.wasm",
    "target/verify-cargo/release/app.wasm",
  ];
  const retained = ["target/tools/bin/just", "target/unowned/data", ".last-cid", ".env", ".websh/local/key", "content/post.md", "docs/notes/draft.md", "dist-note.txt"];
  for (const file of [...generated, ...cached, ...retained]) write(file);

  assert.deepEqual(clean(root, "outputs", true), ["dist", "target/verify", "assets/bundle.css", "dist-preview"]);
  for (const file of [...generated, ...cached, ...retained]) assert.ok(exists(file), file);
  clean(root, "outputs");
  for (const file of generated) assert.ok(!exists(file), file);
  for (const file of [...cached, ...retained]) assert.ok(exists(file), file);
  clean(root, "cache");
  for (const file of cached) assert.ok(!exists(file), file);
  for (const file of retained) assert.ok(exists(file), file);
  assert.deepEqual(clean(root, "cache"), []);
  assert.throws(() => clean(root, "everything"), /Unknown cleanup scope/);
});

test("cleanup rejects redirected paths before deleting any output", (t) => {
  const { root, write, exists } = fixture(t);
  const external = fixture(t);
  write("dist/index.html");
  external.write("verify/keep");
  fs.symlinkSync(external.root, path.join(root, "target"), "dir");

  assert.throws(() => clean(root, "cache"), /Refusing cleanup through symlink/);
  assert.ok(exists("dist/index.html"));
  assert.ok(external.exists("verify/keep"));

  fs.unlinkSync(path.join(root, "target"));
  fs.symlinkSync(external.root, path.join(root, "dist-preview"), "dir");
  assert.throws(() => clean(root, "outputs"), /Refusing cleanup through symlink/);
  assert.ok(exists("dist/index.html"));
  assert.ok(external.exists("verify/keep"));
});
