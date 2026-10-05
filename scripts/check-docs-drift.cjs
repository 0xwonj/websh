#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { publicModules, forbiddenEdges } = require("./architecture.cjs");

const root = path.resolve(__dirname, "..");
const failures = [];

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"], { cwd: root, encoding: "utf8" }));
const workspace = new Set(metadata.workspace_members);
const packages = metadata.packages.filter((pkg) => workspace.has(pkg.id));
const currentArch = read("docs/architecture/current.md");
for (const { name } of packages) {
  if (!currentArch.includes(name)) {
    fail(`docs/architecture/current.md does not mention workspace member ${name}`);
  }
}

for (const edge of forbiddenEdges(packages)) {
  fail(`forbidden workspace dependency direction: ${edge}`);
}

const coreLib = read("crates/websh-core/src/lib.rs");
const facades = publicModules(coreLib);
if (facades.length === 0) fail("core public facade discovery returned no modules");
for (const facade of facades) {
  if (!currentArch.includes(`websh_core::${facade}`)) {
    fail(`docs/architecture/current.md does not mention public facade websh_core::${facade}`);
  }
}

if (failures.length > 0) {
  console.error("docs drift check failed:");
  for (const failure of failures) {
    console.error(`  - ${failure}`);
  }
  process.exit(1);
}

console.log("docs drift check passed");
