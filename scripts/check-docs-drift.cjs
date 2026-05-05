#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const root = process.cwd();
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

function workspaceMembers() {
  const cargo = read("Cargo.toml");
  const match = cargo.match(/members\s*=\s*\[([\s\S]*?)\]/);
  if (!match) {
    fail("Cargo.toml workspace members block not found");
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/g)].map((m) => path.basename(m[1]));
}

function verifyCommands() {
  const justfile = read("justfile");
  const lines = justfile.split(/\r?\n/);
  const out = [];
  let inVerify = false;
  for (const line of lines) {
    if (line.startsWith("verify:")) {
      inVerify = true;
      continue;
    }
    if (!inVerify) {
      continue;
    }
    if (line.length > 0 && !/^\s/.test(line)) {
      break;
    }
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith("#")) {
      out.push(trimmed);
    }
  }
  return out;
}

function verifyRecipe() {
  const justfile = read("justfile");
  const lines = justfile.split(/\r?\n/);
  const out = {
    deps: [],
    commands: [],
  };
  let inVerify = false;
  for (const line of lines) {
    if (line.startsWith("verify:")) {
      inVerify = true;
      out.deps = line
        .slice("verify:".length)
        .trim()
        .split(/\s+/)
        .filter(Boolean);
      continue;
    }
    if (!inVerify) {
      continue;
    }
    if (line.length > 0 && !/^\s/.test(line)) {
      break;
    }
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith("#")) {
      out.commands.push(trimmed);
    }
  }
  return out;
}

function documentedDefaultGate() {
  const doc = read("docs/architecture/verification.md");
  const match = doc.match(
    /The `verify` recipe currently runs:\n\n```bash\n([\s\S]*?)\n```/
  );
  if (!match) {
    fail("docs/architecture/verification.md default gate command block not found");
    return [];
  }
  return match[1].split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
}

function packageWorkspaceEdges(members) {
  const memberSet = new Set(members);
  const edges = [];
  for (const member of members) {
    const cargoPath = path.join(root, "crates", member, "Cargo.toml");
    if (!fs.existsSync(cargoPath)) {
      continue;
    }
    const body = fs.readFileSync(cargoPath, "utf8");
    for (const dep of memberSet) {
      if (dep === member) {
        continue;
      }
      const depPattern = dep.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
      const dependencyRegex = new RegExp(
        `(?:^|\\n)${depPattern}\\s*=\\s*(?:\\{|")`,
        "m"
      );
      if (dependencyRegex.test(body)) {
        edges.push(`${member}->${dep}`);
      }
    }
  }
  return edges.sort();
}

const members = workspaceMembers();
const requiredMemberDocs = [
  "README.md",
  "CLAUDE.md",
  "docs/architecture/current.md",
  "docs/architecture/crates.md",
];

for (const docPath of requiredMemberDocs) {
  const body = read(docPath);
  for (const member of members) {
    if (!body.includes(member)) {
      fail(`${docPath} does not mention workspace member ${member}`);
    }
  }
}

const activeDocs = [
  "README.md",
  "CLAUDE.md",
  ...fs
    .readdirSync(path.join(root, "docs/architecture"))
    .filter((name) => name.endsWith(".md"))
    .map((name) => `docs/architecture/${name}`),
];
const stalePatterns = [
  /(^|[^-\w])src\/components\//,
  /(^|[^-\w])src\/filesystem\//,
  /(^|[^-\w])src\/utils\//,
  /(^|[^-\w])src\/core\//,
  /(^|[^-\w])src\/app\.rs/,
];

for (const docPath of activeDocs) {
  const body = read(docPath);
  for (const pattern of stalePatterns) {
    if (pattern.test(body)) {
      fail(`${docPath} references stale root path pattern ${pattern}`);
    }
  }
}

const verificationDoc = read("docs/architecture/verification.md");
const recipe = verifyRecipe();
for (const dependency of recipe.deps) {
  if (!verificationDoc.includes(dependency)) {
    fail(`docs/architecture/verification.md is missing just verify dependency: ${dependency}`);
  }
}
for (const command of recipe.commands) {
  if (!verificationDoc.includes(command)) {
    fail(`docs/architecture/verification.md is missing just verify command: ${command}`);
  }
}
const documentedGate = documentedDefaultGate();
const expectedGate = recipe.commands;
if (JSON.stringify(documentedGate) !== JSON.stringify(expectedGate)) {
  fail(
    `docs/architecture/verification.md default command block does not exactly match just verify commands`
  );
}

const actualEdges = packageWorkspaceEdges(members);
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
const facades = [...coreLib.matchAll(/^pub mod ([a-z_]+);/gm)].map((m) => m[1]);
const currentArch = read("docs/architecture/current.md");
for (const facade of facades) {
  if (!currentArch.includes(`websh_core::${facade}`)) {
    fail(`docs/architecture/current.md does not mention public facade websh_core::${facade}`);
  }
}

const packageJson = JSON.parse(read("package.json"));
for (const scriptName of ["lint:css", "docs:drift", "perf:budgets", "e2e"]) {
  if (!packageJson.scripts?.[scriptName]) {
    fail(`package.json is missing script ${scriptName}`);
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
