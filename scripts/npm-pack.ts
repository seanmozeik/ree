#!/usr/bin/env -S bun run

import { $ } from "bun";
import fs from "node:fs/promises";
import path from "node:path";
import binaryTemplate from "./binary.js.txt";

interface TargetDefinition {
  label: string;
  triple: string;
  npmOs: "darwin" | "linux";
  npmCpu: "arm64" | "x64";
  libc?: "glibc";
}

interface CargoPackage {
  name: string;
  version: string;
  description?: string;
  license?: string;
  repository?: string;
  homepage?: string;
  targets: Array<{ kind: string[]; name: string }>;
}

const TARGETS: TargetDefinition[] = [
  {
    label: "darwin-arm64",
    npmCpu: "arm64",
    npmOs: "darwin",
    triple: "aarch64-apple-darwin",
  },
  {
    label: "darwin-x64",
    npmCpu: "x64",
    npmOs: "darwin",
    triple: "x86_64-apple-darwin",
  },
  {
    label: "linux-arm64",
    libc: "glibc",
    npmCpu: "arm64",
    npmOs: "linux",
    triple: "aarch64-unknown-linux-gnu",
  },
  {
    label: "linux-x64",
    libc: "glibc",
    npmCpu: "x64",
    npmOs: "linux",
    triple: "x86_64-unknown-linux-gnu",
  },
];

const args = parseArguments(process.argv.slice(2));
const npmOrg = args["npm-org"] ?? process.env.NPM_ORG ?? "seanmozeik";
const maxBytes = Number.parseInt(args["max-bytes"] ?? "600000", 10);
const skipSmoke = args["skip-smoke"] === "true";

function parseArguments(argv: string[]): Record<string, string> {
  const parsed: Record<string, string> = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument?.startsWith("--")) {
      continue;
    }
    const key = argument.slice(2);
    const next = argv[index + 1];
    if (next && !next.startsWith("--")) {
      parsed[key] = next;
      index += 1;
    } else {
      parsed[key] = "true";
    }
  }
  return parsed;
}

function scopedName(name: string): string {
  return `@${npmOrg}/${name}`;
}

function repositoryUrl(repository: string | undefined, packageName: string): string {
  const source = repository ?? `https://github.com/${npmOrg}/${packageName}`;
  const gitUrl = source.startsWith("git+") ? source : `git+${source}`;
  return gitUrl.endsWith(".git") ? gitUrl : `${gitUrl}.git`;
}

async function loadCargoPackage(): Promise<{
  cargoPackage: CargoPackage;
  targetDirectory: string;
}> {
  const metadata = JSON.parse(await $`cargo metadata --format-version 1 --no-deps`.text());
  const binaryPackages = metadata.packages.filter((candidate: CargoPackage) =>
    candidate.targets.some((target) => target.kind.includes("bin")),
  );
  if (binaryPackages.length !== 1) {
    throw new Error(`expected one binary Cargo package, found ${binaryPackages.length}`);
  }
  return {
    cargoPackage: binaryPackages[0],
    targetDirectory: metadata.target_directory,
  };
}

async function packageTarget(
  target: TargetDefinition,
  targetDirectory: string,
  binaryName: string,
  cargoPackage: CargoPackage,
  repository: { type: "git"; url: string },
): Promise<void> {
  const source = path.join(targetDirectory, target.triple, "release", binaryName);
  await fs.access(source);

  const packageDirectory = path.join("npm", target.label);
  const packagedBinary = path.join(packageDirectory, "bin", binaryName);
  await fs.mkdir(path.dirname(packagedBinary), { recursive: true });
  await fs.copyFile(source, packagedBinary);
  await fs.chmod(packagedBinary, 0o755);

  const { size } = await fs.stat(packagedBinary);
  if (size > maxBytes) {
    throw new Error(`${target.label}: ${size} bytes exceeds ${maxBytes}`);
  }

  const manifest: Record<string, unknown> = {
    bin: { [binaryName]: `bin/${binaryName}` },
    cpu: [target.npmCpu],
    description: `Prebuilt ${binaryName} binary for ${target.label}.`,
    files: [`bin/${binaryName}`],
    license: cargoPackage.license,
    name: scopedName(`${binaryName}-${target.label}`),
    os: [target.npmOs],
    private: false,
    publishConfig: { access: "public" },
    repository,
    type: "module",
    version: cargoPackage.version,
  };
  if (target.libc) {
    manifest.libc = [target.libc];
  }
  await Bun.write(
    path.join(packageDirectory, "package.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  console.log(`packaged ${target.label}: ${size} bytes`);

  const isHost = process.platform === target.npmOs && process.arch === target.npmCpu;
  if (!skipSmoke && isHost) {
    await $`${packagedBinary} --version`;
    console.log(`smoke passed: ${target.label}`);
  }
}

async function smokeRootWrapper(binaryName: string): Promise<void> {
  if (skipSmoke) {
    return;
  }
  const host = TARGETS.find(
    (target) => target.npmOs === process.platform && target.npmCpu === process.arch,
  );
  if (!host) {
    throw new Error(`unsupported packaging host: ${process.platform}-${process.arch}`);
  }

  const packageName = scopedName(`${binaryName}-${host.label}`);
  const link = path.join("npm", "node_modules", ...packageName.split("/"));
  await fs.mkdir(path.dirname(link), { recursive: true });
  await fs.symlink(path.resolve("npm", host.label), link, "dir");
  try {
    await $`node npm/binary.js --version`;
  } finally {
    await fs.rm(path.join("npm", "node_modules"), { force: true, recursive: true });
  }
  console.log("root wrapper smoke passed");
}

async function main(): Promise<void> {
  if (!Number.isSafeInteger(maxBytes) || maxBytes <= 0) {
    throw new Error(`invalid --max-bytes value: ${args["max-bytes"]}`);
  }

  const { cargoPackage, targetDirectory } = await loadCargoPackage();
  const binaryTarget = cargoPackage.targets.find((target) => target.kind.includes("bin"));
  if (!binaryTarget) {
    throw new Error(`Cargo package ${cargoPackage.name} has no binary target`);
  }
  const binaryName = args.binary ?? binaryTarget.name;
  const repository = {
    type: "git" as const,
    url: repositoryUrl(cargoPackage.repository, cargoPackage.name),
  };

  await fs.rm("npm", { force: true, recursive: true });
  await fs.mkdir("npm", { recursive: true });
  await Promise.all(
    TARGETS.map((target) =>
      packageTarget(target, targetDirectory, binaryName, cargoPackage, repository),
    ),
  );

  await fs.copyFile("README.md", "npm/README.md");
  await fs.copyFile("LICENSE", "npm/LICENSE");
  const rootManifest = {
    bin: { [binaryName]: "binary.js" },
    description: cargoPackage.description,
    files: ["binary.js", "README.md", "LICENSE"],
    homepage: cargoPackage.homepage ?? cargoPackage.repository,
    license: cargoPackage.license,
    name: scopedName(binaryName),
    optionalDependencies: Object.fromEntries(
      TARGETS.map((target) => [scopedName(`${binaryName}-${target.label}`), cargoPackage.version]),
    ),
    private: false,
    publishConfig: { access: "public" },
    repository,
    type: "module",
    version: cargoPackage.version,
    workspaces: TARGETS.map((target) => target.label),
  };
  await Bun.write("npm/package.json", `${JSON.stringify(rootManifest, null, 2)}\n`);

  const platforms = Object.fromEntries(
    TARGETS.map((target) => [
      `${target.npmOs}:${target.npmCpu}`,
      scopedName(`${binaryName}-${target.label}`),
    ]),
  );
  const resolver = binaryTemplate
    .replace("{{PLATFORMS}}", JSON.stringify(platforms, null, 2))
    .replaceAll("{{BINARY}}", binaryName);
  await Bun.write("npm/binary.js", resolver);
  await fs.chmod("npm/binary.js", 0o755);
  await smokeRootWrapper(binaryName);
  console.log(`npm packages ready: ${scopedName(binaryName)} ${cargoPackage.version}`);
}

await main();
