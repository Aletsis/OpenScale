#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";

const APPLY = process.argv.includes("--apply");

function git(args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

function parseSemver(tag) {
  const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(tag);
  if (!match) return null;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function getLatestSemverTag() {
  const raw = git(["tag", "--list", "v*.*.*", "--sort=-v:refname"]);
  if (!raw) return null;
  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (parseSemver(trimmed)) {
      return trimmed;
    }
  }
  return null;
}

function determineBump(commits) {
  let bump = "patch";
  for (const commit of commits) {
    const [subject = "", ...rest] = commit.split(/\r?\n/);
    const body = rest.join("\n");
    if (
      /^[a-z]+(\([^)]+\))?!:/.test(subject) ||
      /^BREAKING[ -]CHANGE:/m.test(body)
    ) {
      return "major";
    }
    if (/^feat(\([^)]+\))?:/.test(subject)) {
      bump = "minor";
    }
  }
  return bump;
}

function bumpVersion(ver, bump) {
  if (bump === "major") return `${ver.major + 1}.0.0`;
  if (bump === "minor") return `${ver.major}.${ver.minor + 1}.0`;
  return `${ver.major}.${ver.minor}.${ver.patch + 1}`;
}

function updateJsonVersion(filePath, version) {
  if (!existsSync(filePath)) return;
  const content = readFileSync(filePath, "utf8");
  const updated = content.replace(
    /("version"\s*:\s*")([^"]+)(")/,
    `$1${version}$3`,
  );
  if (updated !== content) {
    writeFileSync(filePath, updated, "utf8");
  }
}

function updateTomlSectionVersion(filePath, sectionHeader, version) {
  if (!existsSync(filePath)) return;
  const content = readFileSync(filePath, "utf8");
  const escaped = sectionHeader.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const regex = new RegExp(`(${escaped}[\\s\\S]*?^version\\s*=\\s*")([^"]+)(")`, "m");
  const updated = content.replace(regex, `$1${version}$3`);
  if (updated !== content) {
    writeFileSync(filePath, updated, "utf8");
  }
}

function applyVersion(version) {
  const jsonFiles = [
    "package.json",
    "apps/desktop/package.json",
    "apps/desktop/src-tauri/tauri.conf.json",
    "packages/client/package.json",
    "packages/i18n/package.json",
    "packages/ui/package.json",
  ];
  for (const file of jsonFiles) {
    updateJsonVersion(file, version);
  }
  updateTomlSectionVersion("Cargo.toml", "[workspace.package]", version);
  updateTomlSectionVersion("apps/desktop/src-tauri/Cargo.toml", "[package]", version);
}

function emitOutputs(outputs) {
  const ghOutput = process.env.GITHUB_OUTPUT;
  if (ghOutput) {
    const lines = Object.entries(outputs).map(([k, v]) => `${k}=${v}`);
    appendFileSync(ghOutput, `${lines.join("\n")}\n`, "utf8");
  }
  console.log(JSON.stringify(outputs, null, 2));
}

function main() {
  const latestTag = getLatestSemverTag();

  if (!latestTag) {
    const version = "0.1.0";
    const tag = `v${version}`;
    if (APPLY) {
      applyVersion(version);
    }
    emitOutputs({
      should_release: "true",
      version,
      tag,
      bump: "initial",
      previous_tag: "",
    });
    return;
  }

  const headSha = git(["rev-parse", "HEAD"]);
  const tagSha = git(["rev-list", "-n", "1", latestTag]);
  if (headSha === tagSha) {
    emitOutputs({
      should_release: "false",
      version: latestTag.replace(/^v/, ""),
      tag: latestTag,
      bump: "none",
      previous_tag: latestTag,
    });
    return;
  }

  const rawLog = git(["log", `${latestTag}..HEAD`, "--format=%s%n%b%n---COMMIT-SEP---"]);
  const commits = rawLog
    .split("---COMMIT-SEP---")
    .map((c) => c.trim())
    .filter((c) => c.length > 0 && !c.includes("[skip ci]"));

  if (commits.length === 0) {
    emitOutputs({
      should_release: "false",
      version: latestTag.replace(/^v/, ""),
      tag: latestTag,
      bump: "none",
      previous_tag: latestTag,
    });
    return;
  }

  const current = parseSemver(latestTag);
  const bump = determineBump(commits);
  const version = bumpVersion(current, bump);
  const tag = `v${version}`;

  if (APPLY) {
    applyVersion(version);
  }

  emitOutputs({
    should_release: "true",
    version,
    tag,
    bump,
    previous_tag: latestTag,
  });
}

main();
