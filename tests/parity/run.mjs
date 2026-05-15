#!/usr/bin/env node
/**
 * Layer 4: spec-parity diff.
 *
 * For each scenario, runs the Rust CLI, then independently re-derives the same
 * answer (PowerShell for AppX queries, raw HTTPS for endpoints documented in
 * BEHAVIOR_SPEC.md §10) and asserts they agree on the fields that are supposed
 * to be deterministic.
 *
 * If FLARIAL_NET_CLI is set, also runs the .NET CLI and adds a third diff
 * column. Without it, we still get meaningful spec-conformance signal.
 */
import { spawn, spawnSync } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const ROOT = resolve(dirname(__filename), "..", "..");
const SCENARIOS_DIR = join(ROOT, "tests/parity/scenarios");
const RUST_CLI = join(ROOT, "target/release/flarial-cli.exe");
const NET_CLI = process.env.FLARIAL_NET_CLI ?? null;

function runCli(exe, args) {
  return new Promise((resolve_) => {
    const child = spawn(exe, args, { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let err = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (err += d));
    child.on("close", (code) => resolve_({ code, stdout: out, stderr: err }));
    child.on("error", (e) => resolve_({ code: -1, stdout: "", stderr: e.message }));
  });
}

function pwsh(script) {
  const r = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-NonInteractive", "-Command", script],
    { encoding: "utf8" },
  );
  return r.stdout.trim();
}

async function fetchJson(url) {
  const r = await fetch(url, { headers: { "User-Agent": "flarial-parity-test" } });
  if (!r.ok) throw new Error(`${url} → ${r.status}`);
  const text = await r.text();
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function parseMaybe(s) {
  try { return JSON.parse(s); } catch { return s.trim(); }
}

function deepEq(a, b) {
  return JSON.stringify(a) === JSON.stringify(b);
}

const checks = {
  async package(rustJson) {
    const psPath = pwsh(
      "(Get-AppxPackage Microsoft.MinecraftUWP).InstallLocation",
    );
    const psVersion = pwsh(
      "(Get-AppxPackage Microsoft.MinecraftUWP).Version",
    );
    const installed = !!psPath;
    const truth = {
      installed,
      packaged: installed,
      gaming_services_installed: !!pwsh(
        "(Get-AppxPackage Microsoft.GamingServices).Version",
      ),
      installed_path: psPath || null,
    };
    const issues = [];
    if (rustJson.installed !== truth.installed)
      issues.push(`installed: rust=${rustJson.installed} truth=${truth.installed}`);
    if (rustJson.gaming_services_installed !== truth.gaming_services_installed)
      issues.push(
        `gaming_services: rust=${rustJson.gaming_services_installed} truth=${truth.gaming_services_installed}`,
      );
    if (truth.installed) {
      // installed_path: spec says case-insensitive equal
      if (
        (rustJson.installed_path ?? "").toLowerCase() !==
        truth.installed_path.toLowerCase()
      ) {
        issues.push(
          `installed_path mismatch:\n      rust:  ${rustJson.installed_path}\n      truth: ${truth.installed_path}`,
        );
      }
      // version: rust returns major.minor.(build/100) per spec; PS returns full 4-part
      const psBuild = parseInt(psVersion.split(".")[2] ?? "0", 10);
      const expectedVersion = `${psVersion.split(".")[0]}.${psVersion.split(".")[1]}.${Math.floor(psBuild / 100)}`;
      if (rustJson.version !== expectedVersion) {
        issues.push(
          `version: rust=${rustJson.version} expected=${expectedVersion} (from PS ${psVersion})`,
        );
      }
    }
    return issues;
  },
  async versions(rustJson) {
    const truth = await fetchJson(
      "https://cdn.flarial.xyz/launcher/Supported.json",
    );
    const issues = [];
    const truthKeys = Object.keys(truth).sort();
    const rustKeys = rustJson.map((v) => v.version).sort();
    if (!deepEq(truthKeys, rustKeys)) {
      const missing = truthKeys.filter((k) => !rustKeys.includes(k));
      const extra = rustKeys.filter((k) => !truthKeys.includes(k));
      issues.push(
        `key set mismatch: missing=${missing.join(",")} extra=${extra.join(",")}`,
      );
    }
    for (const v of rustJson) {
      if (truth[v.version] !== v.supported) {
        issues.push(
          `version ${v.version}: rust=${v.supported} truth=${truth[v.version]}`,
        );
      }
    }
    // descending sort check
    const sorted = [...rustJson].sort((a, b) => {
      const pa = a.version.split(".").map(Number);
      const pb = b.version.split(".").map(Number);
      for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
        if ((pb[i] ?? 0) !== (pa[i] ?? 0)) return (pb[i] ?? 0) - (pa[i] ?? 0);
      }
      return 0;
    });
    if (!deepEq(sorted, rustJson)) {
      issues.push("versions are not sorted descending");
    }
    return issues;
  },
  async updates(rustJson) {
    const truth = await fetchJson(
      "https://cdn.flarial.xyz/launcher/launcherVersion.txt",
    );
    // rust returns "X.Y.Z" if remote != local, else null
    if (rustJson === null) {
      // local must equal remote — we can't verify easily here without rebuilding
      return [];
    }
    if (rustJson !== truth.version) {
      return [
        `update version mismatch: rust=${rustJson} truth=${truth.version}`,
      ];
    }
    return [];
  },
};

let failed = 0;
const scenarios = (await readdir(SCENARIOS_DIR)).filter((f) => f.endsWith(".json"));
for (const file of scenarios) {
  const scenario = JSON.parse(await readFile(join(SCENARIOS_DIR, file), "utf8"));
  const sub = scenario.subcommand[0];
  process.stdout.write(`— ${scenario.name} (${sub}) … `);

  const r = await runCli(RUST_CLI, scenario.subcommand);
  if (r.code !== 0) {
    failed++;
    console.log(`FAIL: rust exit=${r.code} stderr=${r.stderr.trim()}`);
    continue;
  }
  const rustJson = parseMaybe(r.stdout);
  const checker = checks[sub];
  if (!checker) {
    console.log("SKIP (no checker)");
    continue;
  }
  const issues = await checker(rustJson);

  let netIssues = [];
  if (NET_CLI) {
    const n = await runCli(NET_CLI, ["--json", ...scenario.subcommand]);
    if (n.code === 0) {
      const netJson = parseMaybe(n.stdout);
      if (!deepEq(rustJson, netJson)) {
        netIssues.push(
          `.NET disagrees:\n      rust: ${JSON.stringify(rustJson)}\n      net:  ${JSON.stringify(netJson)}`,
        );
      }
    } else {
      netIssues.push(`.NET CLI failed: exit=${n.code}`);
    }
  }
  const all = [...issues, ...netIssues];
  if (all.length === 0) {
    console.log("PASS");
  } else {
    failed++;
    console.log("FAIL");
    for (const i of all) console.log(`    ${i}`);
  }
}

if (NET_CLI === null) {
  console.log(
    "\n(No .NET CLI provided — set FLARIAL_NET_CLI=path\\to\\flarial-cli.exe to add the .NET diff column. The spec-parity checks above still validate Rust output against PowerShell + the documented endpoints.)",
  );
}

process.exit(failed === 0 ? 0 : 1);
