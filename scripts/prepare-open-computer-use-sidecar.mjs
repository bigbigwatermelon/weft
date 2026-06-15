#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const sidecarDir = join(repoRoot, "src-tauri", "sidecars");
const runtimeDir = join(sidecarDir, "open-computer-use-runtime");
const runtimeAppName = "Open Computer Use.app";
const runtimeAppPath = join(runtimeDir, runtimeAppName);
const runtimeBinaryPath = join(runtimeAppPath, "Contents", "MacOS", "OpenComputerUse");
const wrapperPath = join(sidecarDir, "open-computer-use");
const metadataPath = join(sidecarDir, "open-computer-use.version.json");
const packageName = "open-computer-use";
const packageVersion = "0.1.53";

const expected = {
  name: packageName,
  npmVersion: packageVersion,
  npmTarball: "https://registry.npmjs.org/open-computer-use/-/open-computer-use-0.1.53.tgz",
  npmShasum: "d740b0c3af25ecc706ca747c1758742a365658ab",
  npmIntegrity:
    "sha512-5qwCPl7Gm4Wk2i/wFkq2dVLN2SzNRQSJTd95zXdGF+u5ZsUXkFx1IFVdNbYelWOpc4fgy8Z8/gYrbacj/2chig==",
  upstream: "https://github.com/iFurySt/open-codex-computer-use",
  gitTag: "v0.1.53",
  pinnedRef: "b753b790cace188152ffb755cd13b2ac9ff6ebf7",
  license: "MIT",
};

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repoRoot,
    encoding: "utf8",
    stdio: options.stdio ?? "pipe",
  });

  if (result.status !== 0) {
    const stdout = result.stdout ? `\nstdout:\n${result.stdout}` : "";
    const stderr = result.stderr ? `\nstderr:\n${result.stderr}` : "";
    const error = result.error ? `\nerror:\n${result.error.message}` : "";
    throw new Error(
      `${command} ${args.join(" ")} failed with exit code ${result.status}${stdout}${stderr}${error}`,
    );
  }

  return result;
}

function hashFile(path, algorithm, encoding) {
  const hash = createHash(algorithm);
  hash.update(readFileSync(path));
  return hash.digest(encoding);
}

function assertEqual(label, actual, expectedValue) {
  if (actual !== expectedValue) {
    throw new Error(`${label} mismatch: expected ${expectedValue}, got ${actual}`);
  }
}

function verifyTarball(path, packInfo) {
  assertEqual("npm package name", packInfo.name, expected.name);
  assertEqual("npm package version", packInfo.version, expected.npmVersion);
  assertEqual("npm shasum from npm pack", packInfo.shasum, expected.npmShasum);
  assertEqual("npm integrity from npm pack", packInfo.integrity, expected.npmIntegrity);

  const actualSha1 = hashFile(path, "sha1", "hex");
  const actualSha512 = hashFile(path, "sha512", "base64");
  const expectedSha512 = expected.npmIntegrity.replace(/^sha512-/, "");

  assertEqual("tarball sha1", actualSha1, expected.npmShasum);
  assertEqual("tarball sha512", actualSha512, expectedSha512);
}

function writeWrapper() {
  const wrapper = `#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "\${BASH_SOURCE[0]}")" && pwd -P)"
RUNTIME="\${DIR}/open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse"

if [[ ! -x "\${RUNTIME}" ]]; then
  echo "Atlas Computer Use runtime is missing or not executable: \${RUNTIME}" >&2
  exit 127
fi

exec "\${RUNTIME}" "$@"
`;

  writeFileSync(wrapperPath, wrapper);
  chmodSync(wrapperPath, 0o755);
}

function writeMetadata() {
  const metadata = {
    name: expected.name,
    upstream: expected.upstream,
    license: expected.license,
    gitTag: expected.gitTag,
    pinnedRef: expected.pinnedRef,
    npmPackage: `${packageName}@${packageVersion}`,
    npmTarball: expected.npmTarball,
    npmShasum: expected.npmShasum,
    npmIntegrity: expected.npmIntegrity,
    binary: "open-computer-use",
    runtime: "open-computer-use-runtime/Open Computer Use.app/Contents/MacOS/OpenComputerUse",
    preparedBy: "scripts/prepare-open-computer-use-sidecar.mjs",
    notes:
      "Atlas bundles the native runtime from the pinned npm release artifact and does not run upstream global installer scripts.",
  };

  writeFileSync(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`);
}

function verifyLocalRuntime() {
  if (!existsSync(wrapperPath)) {
    throw new Error(`Missing wrapper: ${wrapperPath}`);
  }

  if (!existsSync(runtimeBinaryPath)) {
    throw new Error(`Missing runtime binary: ${runtimeBinaryPath}`);
  }

  const versionResult = run(wrapperPath, ["--version"]);
  const version = versionResult.stdout.trim();
  assertEqual("runtime version", version, packageVersion);

  const helpResult = run(wrapperPath, ["help", "call"]);
  if (!helpResult.stdout.includes("open-computer-use call list_apps")) {
    throw new Error("Runtime help output does not expose the expected call command");
  }

  console.log(`Verified ${packageName} ${packageVersion} at ${wrapperPath}`);
}

function prepareSidecar() {
  const tmpRoot = mkdtempSync(join(tmpdir(), "atlas-open-computer-use-"));

  try {
    const pack = run("npm", [
      "pack",
      `${packageName}@${packageVersion}`,
      "--json",
      "--pack-destination",
      tmpRoot,
    ]);

    const packInfo = JSON.parse(pack.stdout)[0];
    const tarballPath = join(tmpRoot, packInfo.filename);
    verifyTarball(tarballPath, packInfo);

    const extractRoot = join(tmpRoot, "extract");
    mkdirSync(extractRoot, { recursive: true });
    run("tar", ["-xzf", tarballPath, "-C", extractRoot]);

    const packageRoot = join(extractRoot, "package");
    const sourceAppPath = join(packageRoot, "dist", runtimeAppName);
    const sourceLicensePath = join(packageRoot, "LICENSE");

    if (!existsSync(sourceAppPath)) {
      throw new Error(`Missing runtime app in npm package: ${sourceAppPath}`);
    }

    rmSync(runtimeDir, { recursive: true, force: true });
    mkdirSync(runtimeDir, { recursive: true });
    cpSync(sourceAppPath, runtimeAppPath, { recursive: true });

    if (existsSync(sourceLicensePath)) {
      cpSync(sourceLicensePath, join(runtimeDir, "LICENSE.open-computer-use"));
    }

    chmodSync(runtimeBinaryPath, 0o755);
    writeWrapper();
    writeMetadata();
    verifyLocalRuntime();
    console.log(`Prepared ${packageName} ${packageVersion}`);
  } finally {
    rmSync(tmpRoot, { recursive: true, force: true });
  }
}

if (process.argv.includes("--verify-only")) {
  verifyLocalRuntime();
} else {
  prepareSidecar();
}
