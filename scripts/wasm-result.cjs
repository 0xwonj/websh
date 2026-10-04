function parseTestResult(output) {
  const terminal = output.trim().split(/\r?\n/).at(-1);
  const summary = terminal?.match(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) filtered out; finished in (\d+(?:\.\d+)?)s$/);
  if (!summary) throw new Error("WASM runner did not finish with a complete test summary");
  const [, status, passed, failed, ignored, filtered] = summary;
  if (status !== "ok" || Number(failed) !== 0) throw new Error(`WASM tests failed: ${terminal}`);
  if (Number(passed) === 0) throw new Error("WASM runner did not execute any passing tests");
  return { passed: Number(passed), ignored: Number(ignored), filtered: Number(filtered) };
}

module.exports = { parseTestResult };
