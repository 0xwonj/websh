#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");

const DEFAULT_DIST_DIR = "target/verify/dist";
const FONT_EXTENSIONS = new Set([".woff2", ".woff", ".ttf", ".otf"]);
const TRUNK_DEV_PATTERNS = [
  {
    label: "Trunk websocket endpoint",
    pattern: /\.well-known\/trunk\/ws/,
  },
  {
    label: "Trunk websocket template placeholder",
    pattern: /__TRUNK_(?:ADDRESS|WS_BASE)__/,
  },
];

const distDir = path.resolve(
  process.argv[2] || process.env.WEBSH_DIST_DIR || DEFAULT_DIST_DIR
);
const jsonMode = parseBoolean(process.env.WEBSH_SIZE_JSON);
const budgets = {
  wasmBrotliBytes: parseBytes(process.env.WEBSH_WASM_BROTLI_BUDGET ?? "1.15MiB"),
  jsBrotliBytes: parseBytes(process.env.WEBSH_JS_BROTLI_BUDGET ?? "90KiB"),
  cssBrotliBytes: parseBytes(process.env.WEBSH_CSS_BROTLI_BUDGET ?? "45KiB"),
  fontBrotliBytes: parseBytes(process.env.WEBSH_FONT_BROTLI_BUDGET ?? "500KiB"),
  vendorBrotliBytes: parseBytes(process.env.WEBSH_VENDOR_BROTLI_BUDGET ?? "400KiB"),
  totalBrotliBytes: parseBytes(process.env.WEBSH_TOTAL_BROTLI_BUDGET ?? "1.70MiB"),
};

function parseBoolean(value) {
  return value === "1" || value === "true" || value === "yes";
}

function parseBytes(value) {
  const match = String(value).trim().match(/^(\d+(?:\.\d+)?)(b|kib|kb|mib|mb)?$/i);
  if (!match) {
    throw new Error(`invalid byte budget: ${value}`);
  }
  const amount = Number(match[1]);
  const unit = (match[2] || "b").toLowerCase();
  const multiplier =
    unit === "mib" || unit === "mb"
      ? 1024 * 1024
      : unit === "kib" || unit === "kb"
        ? 1024
        : 1;
  return Math.round(amount * multiplier);
}

function listFiles(dir) {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...listFiles(fullPath));
    } else if (entry.isFile()) {
      out.push(fullPath);
    } else {
      throw new Error(`unsupported deployment entry: ${fullPath}`);
    }
  }
  return out;
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join("/");
}

function relPath(filePath) {
  return normalizePath(path.relative(distDir, filePath));
}

function brotliSize(buffer) {
  return zlib.brotliCompressSync(buffer, {
    params: {
      [zlib.constants.BROTLI_PARAM_QUALITY]: 11,
    },
  }).length;
}

function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

function assetKind(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  return FONT_EXTENSIONS.has(ext) ? "font" : ext.slice(1) || "other";
}

function auditAsset(filePath) {
  const relativePath = relPath(filePath);
  const asset = {
    path: relativePath,
    kind: assetKind(relativePath),
    bytes: fs.statSync(filePath).size,
  };
  asset.brotliBytes = brotliSize(fs.readFileSync(filePath));
  return asset;
}

function inspectIndexHtml() {
  const indexPath = path.join(distDir, "index.html");
  if (!fs.existsSync(indexPath)) {
    return {
      path: "index.html",
      exists: false,
      hasTrunkDevWebsocket: false,
      matches: [],
    };
  }

  const body = fs.readFileSync(indexPath, "utf8");
  const matches = TRUNK_DEV_PATTERNS.filter(({ pattern }) =>
    pattern.test(body)
  ).map(({ label }) => label);

  return {
    path: "index.html",
    exists: true,
    hasTrunkDevWebsocket: matches.length > 0,
    matches,
  };
}

function totalBytes(assets) {
  return assets.reduce((total, asset) => total + asset.bytes, 0);
}

function brotliSum(assets, predicate) {
  return assets
    .filter(predicate)
    .reduce((total, asset) => total + asset.brotliBytes, 0);
}

function vendorAsset(asset) {
  return asset.path.startsWith("assets/vendor/");
}

function enforceBudget(issues, label, actual, budget) {
  if (actual > budget) {
    issues.push(
      `${label} brotli size ${formatBytes(actual)} exceeds budget ${formatBytes(
        budget
      )}`
    );
  }
}

function buildReport() {
  if (!fs.existsSync(distDir) || !fs.statSync(distDir).isDirectory()) {
    throw new Error(`dist directory not found: ${distDir}`);
  }

  const assets = listFiles(distDir)
    .map(auditAsset)
    .sort((a, b) => {
      if (a.kind !== b.kind) {
        return a.kind.localeCompare(b.kind);
      }
      return b.bytes - a.bytes || a.path.localeCompare(b.path);
    });

  const index = inspectIndexHtml();
  // App releases contain runtime assets only; content is published independently.

  const issues = [];
  if (assets.some((asset) => asset.path.startsWith("content/"))) {
    issues.push("app distribution contains authored content");
  }
  if (!index.exists) {
    issues.push("dist/index.html is missing");
  }
  if (index.hasTrunkDevWebsocket) {
    issues.push(
      `dist/index.html contains Trunk dev websocket code: ${index.matches.join(
        ", "
      )}`
    );
  }
  if (!assets.some((asset) => asset.kind === "wasm")) {
    issues.push("no .wasm asset found in dist");
  }
  const runtime = {
    bytes: totalBytes(assets),
    brotliBytes: brotliSum(assets, () => true),
  };
  enforceBudget(
    issues,
    "wasm",
    brotliSum(assets, (asset) => asset.kind === "wasm"),
    budgets.wasmBrotliBytes
  );
  enforceBudget(
    issues,
    "javascript",
    brotliSum(assets, (asset) => asset.kind === "js"),
    budgets.jsBrotliBytes
  );
  enforceBudget(
    issues,
    "css",
    brotliSum(assets, (asset) => asset.kind === "css"),
    budgets.cssBrotliBytes
  );
  enforceBudget(
    issues,
    "font",
    brotliSum(assets, (asset) => asset.kind === "font"),
    budgets.fontBrotliBytes
  );
  enforceBudget(
    issues,
    "vendor",
    brotliSum(assets, vendorAsset),
    budgets.vendorBrotliBytes
  );
  enforceBudget(issues, "runtime total", runtime.brotliBytes, budgets.totalBrotliBytes);

  return {
    distDir,
    generatedAt: new Date().toISOString(),
    assets,
    runtime,
    index,
    budgets,
    issues,
  };
}

function printHuman(report) {
  console.log(`Asset size audit: ${report.distDir}`);
  console.log("Brotli budgets cover every app release file.");

  console.log("\nAssets:");
  const assets = report.assets;
  for (const kind of new Set(assets.map((asset) => asset.kind))) {
    const group = assets.filter((asset) => asset.kind === kind);
    console.log(`  .${kind}:`);
    const width = Math.max(...group.map((asset) => asset.path.length));
    for (const asset of group) {
      console.log(
        `    ${asset.path.padEnd(width)}  raw=${formatBytes(
          asset.bytes
        ).padStart(9)} brotli=${formatBytes(asset.brotliBytes).padStart(9)}`
      );
    }
  }

  console.log(`\nRuntime: raw=${formatBytes(report.runtime.bytes)} brotli=${formatBytes(report.runtime.brotliBytes)}`);

  if (report.index.hasTrunkDevWebsocket) {
    console.log(
      `\nTrunk dev websocket: present (${report.index.matches.join(", ")})`
    );
  } else if (report.index.exists) {
    console.log("\nTrunk dev websocket: not detected");
  } else {
    console.log("\nTrunk dev websocket: index.html missing");
  }

  if (report.issues.length > 0) {
    console.error("\nIssues:");
    for (const issue of report.issues) {
      console.error(`  - ${issue}`);
    }
  }
}

try {
  const report = buildReport();
  if (jsonMode) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printHuman(report);
  }
  if (report.issues.length > 0) {
    process.exitCode = 1;
  }
} catch (error) {
  if (jsonMode) {
    console.log(
      JSON.stringify(
        {
          distDir,
          error: error.message,
        },
        null,
        2
      )
    );
  } else {
    console.error(error.message);
  }
  process.exit(1);
}
