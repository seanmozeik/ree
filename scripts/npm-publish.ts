#!/usr/bin/env -S bun run

import { spawnSync } from "node:child_process";
import { createInterface } from "node:readline/promises";

type Mode = "all" | "platforms" | "platform" | "root";

const rawArguments = process.argv.slice(2);
const dryRun = rawArguments.includes("--dry-run");
const positional = rawArguments.filter((argument) => argument !== "--dry-run");
const mode = (positional[0] ?? "all") as Mode;
const platform = positional[1];

if (!["all", "platforms", "platform", "root"].includes(mode)) {
  usage(`unknown publish mode: ${mode}`);
}
if (mode === "platform" && !platform) {
  usage("platform mode requires a platform label");
}

const publishFlags = await buildPublishFlags();
switch (mode) {
  case "all":
    publishPlatformWorkspaces();
    publishRoot();
    break;
  case "platforms":
    publishPlatformWorkspaces();
    break;
  case "platform":
    publishSinglePlatform(platform as string);
    break;
  case "root":
    publishRoot();
    break;
}

async function buildPublishFlags(): Promise<string[]> {
  const flags = dryRun ? ["--dry-run"] : [];
  const otp = process.env.NPM_OTP ?? (await promptForOtp());
  if (otp) {
    flags.push("--otp", otp);
  }
  return flags;
}

async function promptForOtp(): Promise<string | null> {
  if (dryRun || !process.stdin.isTTY) {
    return null;
  }
  const prompt = createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await prompt.question("npm OTP (blank to let npm prompt): ");
    return answer.trim() || null;
  } finally {
    prompt.close();
  }
}

function publishPlatformWorkspaces(): void {
  console.log("publishing npm platform workspaces");
  runNpmPublish(["--workspaces"]);
}

function publishSinglePlatform(label: string): void {
  console.log(`publishing npm platform ${label}`);
  runNpmPublish([`./${label}`]);
}

function publishRoot(): void {
  console.log("publishing npm root package");
  runNpmPublish(["."]);
}

function runNpmPublish(arguments_: string[]): void {
  const result = spawnSync("npm", ["publish", ...arguments_, ...publishFlags], {
    cwd: "npm",
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function usage(message: string): never {
  console.error(`error: ${message}`);
  console.error(
    "usage: bun scripts/npm-publish.ts all|platforms|root|platform <label> [--dry-run]",
  );
  process.exit(2);
}
