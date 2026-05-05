#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const DEFAULT_DIST_DIR = "dist";
const REPORT_LIMIT = "30";

const distDir = path.resolve(
  process.argv[2] || process.env.WEBSH_DIST_DIR || DEFAULT_DIST_DIR
);

function findRootWasm() {
  if (!fs.existsSync(distDir) || !fs.statSync(distDir).isDirectory()) {
    throw new Error(`dist directory not found: ${distDir}`);
  }

  const wasmFiles = fs
    .readdirSync(distDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && /_bg\.wasm$/i.test(entry.name))
    .map((entry) => path.join(distDir, entry.name));

  if (wasmFiles.length === 0) {
    throw new Error(`no root *_bg.wasm asset found in ${distDir}`);
  }
  if (wasmFiles.length > 1) {
    throw new Error(
      `expected one root *_bg.wasm asset in ${distDir}, found ${wasmFiles.length}:\n` +
        wasmFiles.map((filePath) => `  - ${path.basename(filePath)}`).join("\n")
    );
  }

  return wasmFiles[0];
}

function commandOutput(command, args) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (result.error) {
    return {
      ok: false,
      output: result.error.code === "ENOENT" ? "not found" : result.error.message,
    };
  }

  return {
    ok: result.status === 0,
    output: `${result.stdout || ""}${result.stderr || ""}`.trim(),
    status: result.status,
  };
}

function requireTwiggy() {
  const version = commandOutput("twiggy", ["--version"]);
  if (!version.ok) {
    throw new Error(
      `twiggy is required for WASM profiling but was not found.\n` +
        `Install it with cargo install twiggy, then rerun npm run perf:wasm-twiggy.`
    );
  }

  return version.output.split(/\r?\n/).find(Boolean) || "twiggy available";
}

function dominatorsLimitOption() {
  const help = commandOutput("twiggy", ["dominators", "--help"]);
  if (help.ok && /^\s+-n(?:\s|,)/m.test(help.output)) {
    return { label: "twiggy dominators -n 30", flag: "-n" };
  }

  return { label: "twiggy dominators -r 30", flag: "-r" };
}

function runTwiggy(label, args) {
  console.log(`\n${label}`);
  console.log("=".repeat(label.length));

  const result = spawnSync("twiggy", args, {
    encoding: "utf8",
    stdio: "inherit",
  });

  if (result.error) {
    throw new Error(result.error.message);
  }
  if (result.status !== 0) {
    throw new Error(`twiggy ${args[0]} exited with status ${result.status}`);
  }
}

try {
  const wasmPath = findRootWasm();
  const version = requireTwiggy();

  console.log(`WASM Twiggy audit: ${wasmPath}`);
  console.log(`Tool: ${version}`);

  runTwiggy("twiggy top -n 30", ["top", "-n", REPORT_LIMIT, wasmPath]);
  const dominatorsLimit = dominatorsLimitOption();
  runTwiggy(dominatorsLimit.label, [
    "dominators",
    dominatorsLimit.flag,
    REPORT_LIMIT,
    wasmPath,
  ]);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
