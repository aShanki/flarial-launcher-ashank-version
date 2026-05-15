import { spawn, ChildProcess, execFileSync } from "node:child_process";
import { resolve } from "node:path";

let tauriDriver: ChildProcess | null = null;

const application = resolve(
  __dirname,
  "../../apps/desktop/src-tauri/target/release/flarial-launcher.exe",
);

export const config = {
  framework: "mocha",
  reporters: ["spec"],
  specs: ["./specs/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [
    {
      "tauri:options": { application },
      browserName: "wry",
    },
  ],
  hostname: "127.0.0.1",
  port: 4444,
  before() {
    execFileSync("cargo", ["build", "--release", "-p", "flarial-launcher"], {
      stdio: "inherit",
    });
  },
  beforeSession() {
    tauriDriver = spawn(
      resolve(process.env.USERPROFILE ?? "", ".cargo", "bin", "tauri-driver.exe"),
      [],
      { stdio: [null, process.stdout, process.stderr] },
    );
  },
  afterSession() {
    tauriDriver?.kill();
  },
  mochaOpts: { ui: "bdd", timeout: 60_000 },
};
