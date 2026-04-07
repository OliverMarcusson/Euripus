import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import process from "node:process";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const scriptsDir = path.join(repoRoot, "scripts");

const command = process.argv[2];

const scriptMap = {
  "dev:start": {
    ps1: "dev-start.ps1",
    sh: "dev-start.sh",
    args: [],
  },
  "dev:stop": {
    ps1: "dev-stop.ps1",
    sh: "dev-stop.sh",
    args: [],
  },
  "dev:reset": {
    ps1: "dev-reset.ps1",
    sh: "dev-reset.sh",
    args: [],
  },
  "prod:start": {
    sh: "deploy.sh",
    args: [],
  },
  "prod:stop": {
    sh: "prod-stop.sh",
    args: [],
  },
  "prod:reset": {
    sh: "prod-reset.sh",
    args: [],
  },
  publish: {
    ps1: "publish-images.ps1",
    sh: "publish-images.sh",
    args: [],
  },
  deploy: {
    sh: "deploy.sh",
    args: [],
  },
  "build:apk": {
    ps1: "build-apk.ps1",
    sh: "build-apk.sh",
    args: [],
  },
};

if (!command || !(command in scriptMap)) {
  console.error(`Unknown script target: ${command ?? "<missing>"}`);
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

if (target.ps1 && binaryExists("pwsh")) {
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

if (target.ps1 && binaryExists("powershell")) {
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

if (target.sh && binaryExists("bash")) {
  const scriptPath = path.join(scriptsDir, target.sh);
  if (!existsSync(scriptPath)) {
    console.error(`Missing bash script: ${scriptPath}`);
    process.exit(1);
  }
  runWith("bash", [scriptPath, ...target.args]);
}

console.error(
  target.ps1
    ? "No supported script runtime found. Install PowerShell or bash."
    : "No supported script runtime found. Install bash.",
);
process.exit(1);
