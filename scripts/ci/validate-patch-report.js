#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const {
  requiredPatchNamesForProfile,
} = require("../patches/registry.js");

const SUCCESS_STATUSES = new Set(["applied", "already-applied"]);
const DEFAULT_PROFILE = "official-dmg-build";
const LEGACY_PROFILE_ALIASES = new Map([["upstream-build", DEFAULT_PROFILE]]);
const KNOWN_PROFILES = new Set([DEFAULT_PROFILE, ...LEGACY_PROFILE_ALIASES.keys()]);

function usage() {
  return "Usage: validate-patch-report.js <patch-report.json> [--profile official-dmg-build]";
}

function parseArgs(argv) {
  let profile = DEFAULT_PROFILE;
  const positional = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--profile") {
      profile = argv[index + 1];
      if (!profile) {
        throw new Error(usage());
      }
      index += 1;
    } else if (arg === "--help" || arg === "-h") {
      console.log(usage());
      process.exit(0);
    } else {
      positional.push(arg);
    }
  }

  if (positional.length !== 1) {
    throw new Error(usage());
  }
  if (!KNOWN_PROFILES.has(profile)) {
    throw new Error(`Unknown patch validation profile: ${profile}`);
  }
  profile = LEGACY_PROFILE_ALIASES.get(profile) ?? profile;

  return { profile, reportPath: positional[0] };
}

function readReport(reportPath) {
  const raw = fs.readFileSync(reportPath, "utf8");
  const report = JSON.parse(raw);
  if (report == null || typeof report !== "object" || !Array.isArray(report.patches)) {
    throw new Error(`Invalid patch report: ${reportPath}`);
  }
  return report;
}

function validateReport(report, profile) {
  const requiredNames = requiredPatchNamesForProfile(profile);
  const entriesByName = new Map();
  const failures = [];

  for (const [index, patch] of report.patches.entries()) {
    if (patch == null || typeof patch !== "object") {
      failures.push(`patch[${index}]: malformed patch entry`);
      continue;
    }
    if (typeof patch.name !== "string" || patch.name.length === 0) {
      failures.push(`patch[${index}]: missing patch name`);
      continue;
    }
    if (typeof patch.status !== "string" || patch.status.length === 0) {
      failures.push(`${patch.name}: missing patch status`);
    }
    if (!entriesByName.has(patch.name)) {
      entriesByName.set(patch.name, []);
    }
    entriesByName.get(patch.name).push(patch);
  }

  for (const [name, entries] of entriesByName) {
    if (entries.length > 1) {
      failures.push(`${name}: duplicate patch entries`);
    }
  }

  for (const name of requiredNames) {
    const patches = entriesByName.get(name);
    if (patches == null) {
      failures.push(`${name}: missing from patch report`);
      continue;
    }
    if (patches.length !== 1 || typeof patches[0].status !== "string") {
      continue;
    }
    const patch = patches[0];
    if (!SUCCESS_STATUSES.has(patch.status)) {
      failures.push(`${name}: ${patch.status}${patch.reason ? ` (${patch.reason})` : ""}`);
    }
  }

  return failures;
}

function main() {
  try {
    const { profile, reportPath } = parseArgs(process.argv.slice(2));
    const report = readReport(reportPath);
    const failures = validateReport(report, profile);
    if (failures.length > 0) {
      console.error(`Required patch validation failed for profile ${profile}:`);
      for (const failure of failures) {
        console.error(`- ${failure}`);
      }
      process.exit(1);
    }
    console.log(`Required patch validation passed for profile ${profile}.`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  KNOWN_PROFILES,
  SUCCESS_STATUSES,
  readReport,
  validateReport,
};
