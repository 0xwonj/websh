const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const { test } = require("node:test");

const auditScript = path.resolve(__dirname, "../../scripts/check-size.cjs");

test("asset budgets count every app file and reject bundled content", (t) => {
  const dist = fs.mkdtempSync(path.join(os.tmpdir(), "websh-assets-"));
  t.after(() => fs.rmSync(dist, { recursive: true, force: true }));
  const files = {
    "index.html": "<!doctype html>",
    "app.wasm": "wasm bytes",
    "assets/vendor/font.woff": "font bytes",
    "assets/sprite.png": "sprite bytes",
    "assets/config.json": "{}",
    "assets/LICENSE": "license bytes",
  };
  for (const [name, body] of Object.entries(files)) {
    const file = path.join(dist, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, body);
  }
  function audit(budget) {
    const env = { ...process.env, WEBSH_SIZE_JSON: "1" };
    for (const key of Object.keys(env)) {
      if (/^WEBSH_.*_BROTLI_BUDGET$/.test(key)) delete env[key];
    }
    if (budget !== undefined) env.WEBSH_TOTAL_BROTLI_BUDGET = String(budget);
    const result = spawnSync(process.execPath, [auditScript, dist], { encoding: "utf8", env });
    assert.ifError(result.error);
    return { status: result.status, report: JSON.parse(result.stdout) };
  }

  const { status, report } = audit();
  assert.equal(status, 0);
  const human = spawnSync(process.execPath, [auditScript, dist], { encoding: "utf8" });
  assert.equal(human.status, 0, human.stderr);
  assert.match(human.stdout, /Runtime: raw=/);
  assert.equal(report.assets.length, Object.keys(files).length);
  assert.equal(report.runtime.bytes, 62);
  for (const asset of report.assets) {
    assert.ok(Object.hasOwn(asset, "brotliBytes"));
    assert.ok(!Object.hasOwn(asset, "gzipBytes"));
  }
  assert.equal(report.assets.find((asset) => asset.path.endsWith(".woff")).kind, "font");
  assert.equal(audit(report.runtime.brotliBytes).status, 0);
  const failed = audit(report.runtime.brotliBytes - 1);
  assert.equal(failed.status, 1);
  assert.match(failed.report.issues[0], /runtime total.*exceeds budget/);

  fs.mkdirSync(path.join(dist, "content"));
  fs.writeFileSync(path.join(dist, "content", "article.md"), "must not ship");
  assert.match(audit().report.issues[0], /app distribution contains authored content/);
  fs.rmSync(path.join(dist, "content"), { recursive: true });

  fs.writeFileSync(path.join(dist, "index.html"), "__TRUNK_ADDRESS__");
  assert.match(audit().report.issues[0], /Trunk dev websocket/);
});
