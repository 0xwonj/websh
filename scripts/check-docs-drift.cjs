#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { publicModules, workspaceEdges } = require("./architecture.cjs");

const root = path.resolve(__dirname, "..");
const failures = [];
const EXPECTED_WORKSPACE_EDGES = new Set([
  "websh-site->websh-core",
  "websh-cli->websh-core",
  "websh-cli->websh-site",
  "websh-web->websh-core",
  "websh-web->websh-site",
]);

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"], { cwd: root, encoding: "utf8" }));
const workspace = new Set(metadata.workspace_members);
const packages = metadata.packages.filter((pkg) => workspace.has(pkg.id));
const members = packages.map((pkg) => pkg.name);
const requiredMemberDocs = [
  "README.md",
  "AGENTS.md",
  "docs/architecture/current.md",
];

for (const docPath of requiredMemberDocs) {
  const body = read(docPath);
  for (const member of members) {
    if (!body.includes(member)) {
      fail(`${docPath} does not mention workspace member ${member}`);
    }
  }
}

const actualEdges = workspaceEdges(packages);
for (const edge of actualEdges) {
  if (!EXPECTED_WORKSPACE_EDGES.has(edge)) {
    fail(`unexpected workspace dependency edge: ${edge}`);
  }
}
for (const edge of EXPECTED_WORKSPACE_EDGES) {
  if (!actualEdges.includes(edge)) {
    fail(`missing expected workspace dependency edge: ${edge}`);
  }
}

const coreLib = read("crates/websh-core/src/lib.rs");
const facades = publicModules(coreLib);
if (facades.length === 0) fail("core public facade discovery returned no modules");
const currentArch = read("docs/architecture/current.md");
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
