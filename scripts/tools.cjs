const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const root = path.resolve(__dirname, "..");
const toolsDir = path.join(root, "target", "tools");
const env = { ...process.env, PATH: `${path.join(toolsDir, "bin")}${path.delimiter}${process.env.PATH}` };
const tools = require("./tools.json");

function wasmBindgenVersion() {
  const lock = fs.readFileSync(path.join(root, "Cargo.lock"), "utf8");
  const version = lock.match(/\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/);
  if (!version) throw new Error("Cargo.lock does not contain wasm-bindgen");
  return version[1];
}

function installedVersion(command) {
  const result = spawnSync(command, ["--version"], { env, encoding: "utf8" });
  return result.status === 0 ? result.stdout.match(/\d+(?:\.\d+){0,2}/)?.[0] : undefined;
}

function requireVersion(command, version) {
  const actual = installedVersion(command);
  if (actual !== version) {
    throw new Error(`${command}: expected ${version}, found ${actual ?? "not installed"}. Run npm run setup.`);
  }
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, env, stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with ${result.status ?? result.signal}`);
}

function requirements() {
  return {
    ...tools,
    "wasm-bindgen-test-runner": { crate: "wasm-bindgen-cli", version: wasmBindgenVersion() },
  };
}

function checkHost() {
  const node = fs.readFileSync(path.join(root, ".node-version"), "utf8").trim();
  const rust = fs.readFileSync(path.join(root, "rust-toolchain.toml"), "utf8").match(/channel = "([^"]+)"/)[1];
  const npm = require("../package.json").packageManager.split("@")[1];
  requireVersion("node", node);
  requireVersion("npm", npm);
  requireVersion("rustc", rust);
}

function check() {
  checkHost();
  for (const [command, { version }] of Object.entries(requirements())) requireVersion(command, version);
  const { chromium } = require("@playwright/test");
  if (!fs.existsSync(chromium.executablePath())) throw new Error("Chromium is missing. Run npm run setup.");
  console.log("Pinned build and browser tools are ready.");
}

function setup() {
  checkHost();
  run("npm", ["ci"]);
  for (const [command, { crate, version, install }] of Object.entries(requirements())) {
    if (installedVersion(command) !== version) {
      if (!crate) throw new Error(`${command}: expected ${version}. ${install}`);
      run("cargo", ["install", "--locked", "--root", toolsDir, "--version", version, crate]);
    }
  }
  run(path.join(root, "node_modules", ".bin", "playwright"), ["install", "chromium"]);
  check();
}

module.exports = { root, env, run, requireVersion, wasmBindgenVersion };

if (require.main === module) {
  try {
    if (process.argv[2] === "setup") setup();
    else if (process.argv[2] === "check") check();
    else throw new Error("Usage: node scripts/tools.cjs setup|check");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
