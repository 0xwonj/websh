const { chromium } = require("playwright");

const baseUrl = normalizeBaseUrl(
  process.env.WEBSH_PERF_BASE_URL ||
    process.env.WEBSH_E2E_BASE_URL ||
    "http://127.0.0.1:4173"
);

const routes = parseRoutes(process.env.WEBSH_PERF_ROUTES) || [
  { name: "home", path: "/#/" },
  { name: "ledger", path: "/#/ledger" },
  { name: "project-md", path: "/#/projects/websh" },
  { name: "talk-md", path: "/#/talks/zk-compilers" },
  {
    name: "pdf",
    path: "/#/talks/evaluating-compiler-optimization-impacts-on-zkvm-performance.pdf",
  },
];

function normalizeBaseUrl(value) {
  return value.replace(/\/+$/, "");
}

function parseRoutes(raw) {
  if (!raw) {
    return null;
  }
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      throw new Error("WEBSH_PERF_ROUTES must be a JSON array");
    }
    return parsed.map((route, index) => {
      if (
        typeof route !== "object" ||
        typeof route.name !== "string" ||
        typeof route.path !== "string"
      ) {
        throw new Error(`invalid route at index ${index}`);
      }
      return { name: route.name, path: route.path };
    });
  } catch (error) {
    throw new Error(`failed to parse WEBSH_PERF_ROUTES: ${error.message}`);
  }
}

function absoluteUrl(routePath) {
  if (/^https?:\/\//.test(routePath)) {
    return routePath;
  }
  return `${baseUrl}${routePath.startsWith("/") ? "" : "/"}${routePath}`;
}

function round(value) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }
  return Math.round(value);
}

function kib(value) {
  if (!value) {
    return "0";
  }
  return `${Math.round(value / 1024)} KiB`;
}

function resourceKey(name) {
  try {
    const url = new URL(name);
    return `${url.origin}${url.pathname}`;
  } catch (_) {
    return name.split("#")[0].split("?")[0];
  }
}

function summarizeResources(resources) {
  const byKey = new Map();
  for (const entry of resources) {
    const key = resourceKey(entry.name);
    byKey.set(key, (byKey.get(key) || 0) + 1);
  }

  const duplicateContent = [...byKey.entries()]
    .filter(([key, count]) => count > 1 && /\/content\//.test(key))
    .map(([key, count]) => ({ key, count }));

  const pick = (pattern) => resources.filter((entry) => pattern.test(entry.name));

  return {
    wasm: pick(/\.wasm(?:$|[?#])/),
    katex: pick(/\/katex(?:\.min)?\.(?:js|css)(?:$|[?#])/),
    fonts: pick(/\.(?:woff2?|ttf)(?:$|[?#])/),
    ttfFonts: pick(/\.ttf(?:$|[?#])/),
    manifest: pick(/\/manifest\.json(?:$|[?#])/),
    markdown: pick(/\.md(?:$|[?#])/),
    pdf: pick(/\.pdf(?:$|[?#])/),
    content: pick(/\/content\//),
    duplicateContent,
  };
}

function sameOriginFailure(response) {
  const url = new URL(response.url());
  const origin = new URL(baseUrl).origin;
  if (url.origin !== origin) {
    return null;
  }
  if (url.pathname.startsWith("/.well-known/trunk/")) {
    return null;
  }
  if (response.status() < 400) {
    return null;
  }
  return `${response.status()} ${url.pathname}`;
}

function compactEntry(entry) {
  return {
    name: entry.name,
    initiator: entry.initiatorType,
    startMs: round(entry.startTime),
    durationMs: round(entry.duration),
    ttfbMs: round(entry.responseStart - entry.startTime),
    transfer: entry.transferSize,
    encoded: entry.encodedBodySize,
    decoded: entry.decodedBodySize,
  };
}

function printRoute(result) {
  const nav = result.navigation;
  console.log(`\n${result.name} ${result.url}`);
  console.log(
    `  document: ttfb=${round(nav.responseStart)}ms dcl=${round(
      nav.domContentLoadedEventEnd
    )}ms load=${round(nav.loadEventEnd)}ms total=${round(nav.duration)}ms`
  );

  for (const [label, entries] of Object.entries({
    wasm: result.summary.wasm,
    katex: result.summary.katex,
    fonts: result.summary.fonts,
    manifest: result.summary.manifest,
    markdown: result.summary.markdown,
    pdf: result.summary.pdf,
  })) {
    if (entries.length === 0) {
      continue;
    }
    for (const entry of entries) {
      const url = new URL(entry.name);
      console.log(
        `  ${label}: ${url.pathname} duration=${entry.durationMs}ms ttfb=${entry.ttfbMs}ms transfer=${kib(
          entry.transfer
        )} decoded=${kib(entry.decoded)}`
      );
    }
  }

  if (result.summary.duplicateContent.length > 0) {
    console.log("  duplicate content requests:");
    for (const duplicate of result.summary.duplicateContent) {
      console.log(`    ${duplicate.count}x ${duplicate.key}`);
    }
  }

  if (result.summary.ttfFonts.length > 0) {
    console.log("  loaded ttf fonts:");
    for (const entry of result.summary.ttfFonts) {
      console.log(`    ${entry.name}`);
    }
  }

  if (result.networkFailures.length > 0) {
    console.log("  same-origin failures:");
    for (const failure of result.networkFailures) {
      console.log(`    ${failure}`);
    }
  }
}

async function collectRoute(page, route) {
  const networkFailures = [];
  page.on("response", (response) => {
    const failure = sameOriginFailure(response);
    if (failure) {
      networkFailures.push(failure);
    }
  });

  const url = absoluteUrl(route.path);
  await page.goto(url, { waitUntil: "domcontentloaded", timeout: 45000 });
  await page.waitForLoadState("networkidle", { timeout: 20000 }).catch(() => {});
  await page.waitForTimeout(250);

  const timing = await page.evaluate(() => {
    const navigation = performance.getEntriesByType("navigation")[0].toJSON();
    const resources = performance
      .getEntriesByType("resource")
      .map((entry) => entry.toJSON());
    return { navigation, resources };
  });

  const resources = timing.resources.map(compactEntry);
  return {
    name: route.name,
    url,
    navigation: timing.navigation,
    resources,
    networkFailures,
    summary: summarizeResources(resources),
  };
}

(async () => {
  const browser = await chromium.launch({ headless: true });
  const results = [];
  try {
    for (const route of routes) {
      const page = await browser.newPage();
      try {
        const result = await collectRoute(page, route);
        results.push(result);
        printRoute(result);
      } finally {
        await page.close();
      }
    }
  } finally {
    await browser.close();
  }

  if (process.env.WEBSH_PERF_JSON) {
    console.log(JSON.stringify(results, null, 2));
  }

  const failures = [];
  for (const result of results) {
    for (const failure of result.networkFailures) {
      failures.push(`${result.name}: ${failure}`);
    }
    for (const duplicate of result.summary.duplicateContent) {
      failures.push(`${result.name}: duplicate content request ${duplicate.count}x ${duplicate.key}`);
    }
    if (result.summary.wasm.length > 1) {
      failures.push(`${result.name}: loaded wasm ${result.summary.wasm.length} times`);
    }
  }

  if (failures.length > 0) {
    console.error("\ncontent-load failures:");
    for (const failure of failures) {
      console.error(`  ${failure}`);
    }
    process.exit(1);
  }
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
