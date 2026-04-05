import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const scriptsDir = path.join(repoRoot, "scripts");

const command = process.argv[2];

const scriptMap = {
  "user-test:start": {
    ps1: "start-user-test-stack.ps1",
    sh: "start-user-test-stack.sh",
    args: [],
  },
  "user-test:start:web": {
    ps1: "start-user-test-stack.ps1",
    sh: "start-user-test-stack.sh",
    args: ["--web-only", "--open-browser"],
  },
  "user-test:start:dev": {
    ps1: "start-user-test-stack.ps1",
    sh: "start-user-test-stack.sh",
    args: [],
  },
  "user-test:start:dev:web": {
    ps1: "start-user-test-stack.ps1",
    sh: "start-user-test-stack.sh",
    args: ["--web-only", "--open-browser"],
  },
  "user-test:stop": {
    ps1: "stop-user-test-stack.ps1",
    sh: "stop-user-test-stack.sh",
    args: [],
  },
};

if (!command || !(command in scriptMap)) {
  console.error(`Unknown dev script target: ${command ?? "<missing>"}`);
  process.exit(1);
}

function binaryExists(cmd) {
  const checker = process.platform === "win32" ? "where" : "command";
  const checkerArgs = process.platform === "win32" ? [cmd] : ["-v", cmd];
  const result = spawnSync(checker, checkerArgs, {
    stdio: "ignore",
    shell: process.platform !== "win32",
  });
  return result.status === 0;
}

function runWith(executable, args) {
  const result = spawnSync(executable, args, {
    cwd: repoRoot,
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }

  process.exit(result.status ?? 1);
}

const target = scriptMap[command];

if (binaryExists("pwsh")) {
  runWith("pwsh", [
    "-NoLogo",
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    path.join(scriptsDir, target.ps1),
    ...target.args.map((arg) =>
      arg.startsWith("--")
        ? `-${arg
            .slice(2)
            .split("-")
            .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
            .join("")}`
        : arg,
    ),
  ]);
}

if (binaryExists("powershell")) {
  runWith("powershell", [
    "-NoLogo",
    "-NoProfile",
    "-NonInteractive",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    path.join(scriptsDir, target.ps1),
    ...target.args.map((arg) =>
      arg.startsWith("--")
        ? `-${arg
            .slice(2)
            .split("-")
            .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
            .join("")}`
        : arg,
    ),
  ]);
}

if (binaryExists("bash")) {
  const scriptPath = path.join(scriptsDir, target.sh);
  if (!existsSync(scriptPath)) {
    console.error(`Missing bash script: ${scriptPath}`);
    process.exit(1);
  }
  runWith("bash", [scriptPath, ...target.args]);
}

console.error("No supported script runtime found. Install PowerShell or bash.");
process.exit(1);
