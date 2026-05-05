#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");
const { spawnSync } = require("node:child_process");

const DEFAULT_DIST_DIR = "dist";
const TARGET_EXTENSIONS = new Set([".wasm", ".js", ".css", ".woff2", ".ttf"]);
const TRUNK_DEV_PATTERNS = [
  {
    label: "Trunk websocket endpoint",
    pattern: /\.well-known\/trunk\/ws/,
  },
  {
    label: "Trunk websocket template placeholder",
    pattern: /__TRUNK_(?:ADDRESS|WS_BASE)__/,
  },
  {
    label: "Trunk dev WebSocket client",
    pattern: /\.well-known\/trunk\/ws[\s\S]*new\s+WebSocket\(/,
  },
];

const distDir = path.resolve(
  process.argv[2] || process.env.WEBSH_DIST_DIR || DEFAULT_DIST_DIR
);
const jsonMode = parseBoolean(process.env.WEBSH_SIZE_JSON);
const budgets = {
  wasmBrotliBytes: parseOptionalBytes(process.env.WEBSH_WASM_BROTLI_BUDGET),
  jsBrotliBytes: parseOptionalBytes(process.env.WEBSH_JS_BROTLI_BUDGET),
  cssBrotliBytes: parseOptionalBytes(process.env.WEBSH_CSS_BROTLI_BUDGET),
  fontBrotliBytes: parseOptionalBytes(process.env.WEBSH_FONT_BROTLI_BUDGET),
  vendorBrotliBytes: parseOptionalBytes(process.env.WEBSH_VENDOR_BROTLI_BUDGET),
  totalBrotliBytes: parseOptionalBytes(process.env.WEBSH_TOTAL_BROTLI_BUDGET),
};

function parseBoolean(value) {
  return value === "1" || value === "true" || value === "yes";
}

function parseOptionalBytes(value) {
  if (!value) {
    return null;
  }
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

function gzipSize(buffer) {
  return zlib.gzipSync(buffer, { level: 9 }).length;
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
  const ext = path.extname(filePath).slice(1);
  return ext === "woff2" || ext === "ttf" ? "font" : ext;
}

function hasTrunkHash(relativePath) {
  return /-[0-9a-f]{8,}(?:_bg)?\.(?:css|js|wasm)$/i.test(relativePath);
}

function auditAsset(filePath) {
  const buffer = fs.readFileSync(filePath);
  const relativePath = relPath(filePath);
  const rootAsset = !relativePath.includes("/");
  return {
    path: relativePath,
    kind: assetKind(relativePath),
    rootAsset,
    trunkHashed: rootAsset ? hasTrunkHash(relativePath) : null,
    bytes: buffer.length,
    gzipBytes: gzipSize(buffer),
    brotliBytes: brotliSize(buffer),
  };
}

function commandAvailability(command, args) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  if (result.error) {
    return {
      command,
      available: false,
      version: null,
      note: result.error.code === "ENOENT" ? "not found" : result.error.message,
    };
  }

  const output = `${result.stdout || ""}${result.stderr || ""}`.trim();
  return {
    command,
    available: result.status === 0,
    version: output.split(/\r?\n/).find(Boolean) || null,
    note: result.status === 0 ? null : `exited ${result.status}`,
  };
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

function sumAssets(assets) {
  return assets.reduce(
    (acc, asset) => {
      acc.bytes += asset.bytes;
      acc.gzipBytes += asset.gzipBytes;
      acc.brotliBytes += asset.brotliBytes;
      return acc;
    },
    { bytes: 0, gzipBytes: 0, brotliBytes: 0 }
  );
}

function brotliSum(assets, predicate) {
  return assets
    .filter(predicate)
    .reduce((total, asset) => total + asset.brotliBytes, 0);
}

function vendorAsset(asset) {
  return (
    asset.path.includes("/vendor/") ||
    asset.path.startsWith("vendor/") ||
    /(?:vendor|third[-_]party)/i.test(asset.path)
  );
}

function enforceBudget(issues, label, actual, budget) {
  if (budget !== null && actual > budget) {
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
    .filter((filePath) => TARGET_EXTENSIONS.has(path.extname(filePath)))
    .map(auditAsset)
    .sort((a, b) => {
      if (a.kind !== b.kind) {
        return a.kind.localeCompare(b.kind);
      }
      return b.bytes - a.bytes || a.path.localeCompare(b.path);
    });

  const index = inspectIndexHtml();
  const tools = [
    commandAvailability("twiggy", ["--version"]),
    commandAvailability("wasm-opt", ["--version"]),
  ];

  const issues = [];
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
  const wasmBrotli = assets
    .filter((asset) => asset.kind === "wasm")
    .reduce((total, asset) => total + asset.brotliBytes, 0);
  const totals = sumAssets(assets);
  enforceBudget(issues, "wasm", wasmBrotli, budgets.wasmBrotliBytes);
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
  enforceBudget(issues, "total", totals.brotliBytes, budgets.totalBrotliBytes);

  return {
    distDir,
    generatedAt: new Date().toISOString(),
    assets,
    totals,
    index,
    tools,
    budgets,
    issues,
  };
}

function printHuman(report) {
  console.log(`WASM size audit: ${report.distDir}`);

  console.log("\nTool availability:");
  for (const tool of report.tools) {
    const status = tool.available ? "available" : "missing";
    const detail = tool.version || tool.note || "";
    console.log(`  ${tool.command}: ${status}${detail ? ` (${detail})` : ""}`);
  }

  console.log("\nAssets:");
  for (const kind of ["wasm", "js", "css", "font"]) {
    const assets = report.assets.filter((asset) => asset.kind === kind);
    if (assets.length === 0) {
      console.log(`  .${kind}: none`);
      continue;
    }

    console.log(`  .${kind}:`);
    const width = Math.max(...assets.map((asset) => asset.path.length));
    for (const asset of assets) {
      const hashStatus =
        asset.trunkHashed === null
          ? ""
          : asset.trunkHashed
            ? " hashed"
            : " unhashed-root";
      console.log(
        `    ${asset.path.padEnd(width)}  raw=${formatBytes(
          asset.bytes
        ).padStart(9)} gzip=${formatBytes(asset.gzipBytes).padStart(
          9
        )} brotli=${formatBytes(asset.brotliBytes).padStart(9)}${hashStatus}`
      );
    }
  }

  console.log(
    `\nTotals: raw=${formatBytes(report.totals.bytes)} gzip=${formatBytes(
      report.totals.gzipBytes
    )} brotli=${formatBytes(report.totals.brotliBytes)}`
  );

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
