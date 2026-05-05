const { spawn, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const CRATE_PATH = "crates/websh-web";
const DRIVER_UNAVAILABLE = [
  "chromedriver binaries are unavailable",
  "failed to get chromedriver",
  "chromedriver not found",
];

function run(cmd, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      cwd: process.cwd(),
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";

    child.stdout.on("data", (chunk) => {
      const text = chunk.toString();
      output += text;
      process.stdout.write(text);
    });
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString();
      output += text;
      process.stderr.write(text);
    });
    child.on("error", reject);
    child.on("close", (code, signal) => resolve({ code, signal, output }));
  });
}

async function tryHeadlessChrome() {
  if (!commandExists("wasm-pack", ["--version"])) {
    console.warn(
      "wasm-pack is not installed; falling back to the wasm-bindgen test page via Playwright."
    );
    return false;
  }

  const result = await run("wasm-pack", [
    "test",
    "--headless",
    "--chrome",
    CRATE_PATH,
  ]);
  if (result.code === 0) {
    return true;
  }

  const unavailable = DRIVER_UNAVAILABLE.some((needle) =>
    result.output.toLowerCase().includes(needle)
  );
  if (!unavailable) {
    process.exit(result.code ?? 1);
  }

  console.warn(
    "ChromeDriver is unavailable here; falling back to the wasm-bindgen test page via Playwright."
  );
  return false;
}

function commandExists(cmd, args) {
  const result = spawnSync(cmd, args, {
    cwd: process.cwd(),
    env: process.env,
    stdio: "ignore",
  });
  return !result.error && result.status === 0;
}

function startInteractiveServer() {
  return new Promise((resolve, reject) => {
    let runner;
    try {
      runner = findWasmBindgenTestRunner();
    } catch (error) {
      reject(error);
      return;
    }

    const child = spawn(
      "cargo",
      ["test", "-p", "websh-web", "--target", "wasm32-unknown-unknown"],
      {
        cwd: process.cwd(),
        env: {
          ...process.env,
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER: runner,
          NO_HEADLESS: "1",
          WASM_BINDGEN_TEST_ONLY_WEB: "1",
        },
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
      }
    );
    let output = "";
    let resolved = false;

    const handleChunk = (chunk, stream) => {
      const text = chunk.toString();
      output += text;
      stream.write(text);

      const match = text.match(/available at (http:\/\/127\.0\.0\.1:\d+)/);
      if (match && !resolved) {
        resolved = true;
        resolve({ child, url: match[1] });
      }
    };

    child.stdout.on("data", (chunk) => handleChunk(chunk, process.stdout));
    child.stderr.on("data", (chunk) => handleChunk(chunk, process.stderr));
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (!resolved) {
        reject(
          new Error(
            `wasm-bindgen test server exited before it was ready (code=${code}, signal=${signal})\n${output}`
          )
        );
      }
    });
  });
}

function findWasmBindgenTestRunner() {
  const cacheRoots = [
    path.join(os.homedir(), "Library", "Caches", ".wasm-pack"),
    path.join(os.homedir(), ".cache", ".wasm-pack"),
  ].filter((dir) => fs.existsSync(dir));
  const matches = [];

  for (const cacheRoot of cacheRoots) {
    collectRunners(cacheRoot, matches, 0);
  }

  matches.sort((left, right) => right.mtimeMs - left.mtimeMs);
  if (matches.length === 0) {
    throw new Error(
      "wasm-bindgen-test-runner was not found. Install wasm-pack with `cargo install wasm-pack`, or run wasm-pack once so the runner is available in its cache."
    );
  }
  return matches[0].path;
}

function collectRunners(dir, matches, depth) {
  if (depth > 4) {
    return;
  }

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      collectRunners(fullPath, matches, depth + 1);
      continue;
    }

    if (entry.isFile() && entry.name === "wasm-bindgen-test-runner") {
      const stat = fs.statSync(fullPath);
      matches.push({ path: fullPath, mtimeMs: stat.mtimeMs });
    }
  }
}

function stopServer(child) {
  return new Promise((resolve) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      resolve();
      return;
    }

    const signalTree = (signal) => {
      try {
        process.kill(-child.pid, signal);
      } catch (_) {
        child.kill(signal);
      }
    };

    const timeout = setTimeout(() => {
      signalTree("SIGKILL");
      resolve();
    }, 2000);

    child.once("close", () => {
      clearTimeout(timeout);
      resolve();
    });
    signalTree("SIGTERM");
  });
}

async function runWithPlaywright() {
  const { child, url } = await startInteractiveServer();
  let browser;

  try {
    const { chromium } = require("playwright");
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    await page.goto(url, { waitUntil: "load" });
    await page.waitForFunction(
      () => document.body && document.body.innerText.includes("test result:"),
      null,
      { timeout: 30000 }
    );
    const bodyText = await page.textContent("body");
    const resultLines = bodyText
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(
        (line) =>
          line.startsWith("running ") ||
          line.startsWith("test ") ||
          line.startsWith("test result:")
      );
    console.log(resultLines.length > 0 ? resultLines.join("\n") : bodyText);

    if (!/test result: ok\./.test(bodyText)) {
      throw new Error("wasm browser tests did not report success");
    }
  } finally {
    if (browser) {
      await browser.close();
    }
    await stopServer(child);
  }
}

(async () => {
  if (await tryHeadlessChrome()) {
    return;
  }
  await runWithPlaywright();
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
