#!/usr/bin/env node
/**
 * Asserts the freshly-built NSIS installer is under the size budget.
 * Exits non-zero with a human-readable diff if it isn't.
 */
import { readdir, stat } from "node:fs/promises";
import { join } from "node:path";

const BUDGET_BYTES = 12 * 1024 * 1024; // 12 MB hard cap (10 MB target + 20% slack)
const ROOT = new URL("..", import.meta.url).pathname.replace(/^\//, "");
const NSIS_DIR = join(ROOT, "target/release/bundle/nsis");

try {
  const entries = await readdir(NSIS_DIR);
  const setups = entries.filter((f) => f.endsWith("-setup.exe"));
  if (setups.length === 0) {
    console.error(`no installer found in ${NSIS_DIR}; run \`pnpm tauri build\` first`);
    process.exit(2);
  }
  let failed = false;
  for (const f of setups) {
    const p = join(NSIS_DIR, f);
    const { size } = await stat(p);
    const mb = (size / 1024 / 1024).toFixed(2);
    const ok = size <= BUDGET_BYTES;
    if (!ok) failed = true;
    console.log(`${ok ? "OK " : "FAIL"} ${f}: ${mb} MB (budget 12.00 MB)`);
  }
  process.exit(failed ? 1 : 0);
} catch (err) {
  console.error(`size check failed: ${err.message}`);
  process.exit(2);
}
