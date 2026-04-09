import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { emitKeypressEvents } from "node:readline";
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

const apkArchitectureChoices = [
  {
    label: "x86 (x86_64)",
    value: "x86_64",
    description: "For x86 Android TV devices and custom x86_64 boxes",
  },
  {
    label: "arm64",
    value: "arm64-v8a",
    description: "For most modern ARM64 Android TV devices",
  },
];

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

function renderRadioPicker(title, choices, selectedIndex) {
  process.stdout.write("\x1Bc");
  console.log(title);
  console.log("");
  choices.forEach((choice, index) => {
    const selected = index === selectedIndex;
    console.log(`${selected ? "(*) " : "( ) "}${choice.label}`);
    console.log(`    ${choice.description}`);
  });
  console.log("");
  console.log("Use ↑/↓ and Enter to choose.");
}

async function promptForApkArchitecture() {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new Error(
      "Architecture selection requires an interactive terminal. Pass --architecture <x86_64|arm64-v8a> instead.",
    );
  }

  return await new Promise((resolve, reject) => {
    let selectedIndex = 0;
    const stdin = process.stdin;
    const wasRaw = stdin.isRaw;

    function cleanup() {
      stdin.off("keypress", handleKeypress);
      if (!wasRaw && stdin.isTTY) {
        stdin.setRawMode(false);
      }
      stdin.pause();
      process.stdout.write("\x1Bc");
    }

    function handleKeypress(_, key) {
      if (!key) {
        return;
      }
      if (key.name === "up") {
        selectedIndex =
          (selectedIndex - 1 + apkArchitectureChoices.length) %
          apkArchitectureChoices.length;
        renderRadioPicker("Choose APK architecture", apkArchitectureChoices, selectedIndex);
        return;
      }
      if (key.name === "down") {
        selectedIndex = (selectedIndex + 1) % apkArchitectureChoices.length;
        renderRadioPicker("Choose APK architecture", apkArchitectureChoices, selectedIndex);
        return;
      }
      if (key.name === "return") {
        const selectedChoice = apkArchitectureChoices[selectedIndex];
        cleanup();
        console.log(`Selected APK architecture: ${selectedChoice.label}`);
        resolve(selectedChoice.value);
        return;
      }
      if (key.ctrl && key.name === "c") {
        cleanup();
        reject(new Error("Architecture selection cancelled."));
      }
    }

    emitKeypressEvents(stdin);
    if (!wasRaw && stdin.isTTY) {
      stdin.setRawMode(true);
    }
    stdin.resume();
    stdin.on("keypress", handleKeypress);
    renderRadioPicker("Choose APK architecture", apkArchitectureChoices, selectedIndex);
  });
}

const target = structuredClone(scriptMap[command]);
const forwardedArgs = process.argv.slice(3);

function takeFlagValue(flagNames) {
  const index = forwardedArgs.findIndex((arg) => flagNames.includes(arg));
  if (index === -1) {
    return null;
  }
  const value = forwardedArgs[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`Missing value for ${forwardedArgs[index]}`);
  }
  forwardedArgs.splice(index, 2);
  return value;
}

if (command === "build:apk") {
  const requestedArchitecture =
    takeFlagValue(["--architecture", "-a"]) ?? process.env.EURIPUS_TARGET_ABI ?? await promptForApkArchitecture();
  target.args = ["--architecture", requestedArchitecture, ...target.args];
}

if (forwardedArgs.length > 0) {
  target.args = [...target.args, ...forwardedArgs];
}

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
