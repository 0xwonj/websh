const { spawn } = require("node:child_process");
const { env, requireVersion, wasmBindgenVersion } = require("../scripts/tools.cjs");
const { parseTestResult } = require("../scripts/wasm-result.cjs");

function startInteractiveServer() {
  return new Promise((resolve, reject) => {
    const startupMs = Number(env.WEBSH_WASM_STARTUP_TIMEOUT_MS || 600000);
    if (!Number.isSafeInteger(startupMs) || startupMs <= 0 || startupMs > 2147483647) {
      reject(new Error("WEBSH_WASM_STARTUP_TIMEOUT_MS must be a positive timer duration"));
      return;
    }
    let runner;
    try {
      requireVersion("wasm-bindgen-test-runner", wasmBindgenVersion());
      runner = "wasm-bindgen-test-runner";
    } catch (error) {
      reject(error);
      return;
    }

    const child = spawn(
      "cargo",
      ["test", "--locked", "-p", "websh-web", "--target", "wasm32-unknown-unknown"],
      {
        cwd: process.cwd(),
        env: {
          ...env,
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER: runner,
          NO_HEADLESS: "1",
          WASM_BINDGEN_TEST_ONLY_WEB: "1",
        },
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
      }
    );
    let output = "";
    let settled = false;
    const fail = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(startup);
      stopServer(child).then(() => reject(error));
    };
    const startup = setTimeout(() => fail(new Error(
      `WASM runner startup exceeded ${startupMs}ms\n${output.slice(-8000)}`
    )), startupMs);

    const handleChunk = (chunk, stream) => {
      const text = chunk.toString();
      output += text;
      stream.write(text);

      const match = output.match(/available at (http:\/\/127\.0\.0\.1:\d+)/);
      if (match && !settled) {
        settled = true;
        clearTimeout(startup);
        resolve({ child, url: match[1] });
      }
    };

    child.stdout.on("data", (chunk) => handleChunk(chunk, process.stdout));
    child.stderr.on("data", (chunk) => handleChunk(chunk, process.stderr));
    child.on("error", fail);
    child.on("close", (code, signal) => {
      if (!settled) {
        fail(
          new Error(
            `wasm-bindgen test server exited before it was ready (code=${code}, signal=${signal})\n${output}`
          )
        );
      }
    });
  });
}

function stopServer(child) {
  return new Promise((resolve) => {
    if (!child.pid) {
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

    if (child.exitCode !== null || child.signalCode !== null) {
      signalTree("SIGKILL");
      resolve();
      return;
    }

    const timeout = setTimeout(() => {
      signalTree("SIGKILL");
      resolve();
    }, 2000);

    child.once("close", () => {
      clearTimeout(timeout);
      // Cargo can exit before a child runner; finish the owned process group.
      signalTree("SIGKILL");
      resolve();
    });
    signalTree("SIGTERM");
  });
}

async function runWithPlaywright() {
  const { child, url } = await startInteractiveServer();
  let browser;

  try {
    const { chromium } = require("@playwright/test");
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    // Wait for the runner's actual promise, not text a test could print early.
    await page.route("**/run.js", async (route) => {
      const response = await route.fetch();
      const source = await response.text();
      if (!/\bmain\(tests\);\s*$/.test(source)) {
        await route.fulfill({ status: 500, body: "Unsupported wasm-bindgen runner bootstrap" });
        return;
      }
      const body = source.replace(/\bmain\(tests\);\s*$/, `main(tests).then(
        () => { window.__webshTestResult = { complete: true }; },
        error => { window.__webshTestResult = { complete: true, error: String(error) }; }
      );`);
      await route.fulfill({ response, body });
    });
    await page.goto(url, { waitUntil: "load" });
    await page.waitForFunction(
      () => window.__webshTestResult?.complete,
      null,
      { timeout: 30000 }
    );
    const runnerError = await page.evaluate(() => window.__webshTestResult.error);
    if (runnerError) throw new Error(`WASM runner rejected: ${runnerError}`);
    const bodyText = await page.textContent("#output");
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

    parseTestResult(bodyText);
  } finally {
    try {
      if (browser) await browser.close();
    } finally {
      await stopServer(child);
    }
  }
}

runWithPlaywright().catch((error) => {
  console.error(error);
  process.exit(1);
});
