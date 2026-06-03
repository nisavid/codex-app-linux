"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { applyLinuxFastModeModelGuardPatch } = require("../../../../webview-assets.js");

function hasFastModeModelGuardCandidate(source) {
  if (!source.includes("serviceTiers")) {
    return false;
  }
  return source.includes("additionalSpeedTiers") ||
    source.includes("serviceTier.fast.label") ||
    source.includes("serviceTier.ultrafast.label") ||
    source.includes("defaultServiceTier");
}

function applyLinuxFastModeModelGuardPatchToExtractedApp(extractedDir) {
  const webviewAssetsDir = path.join(extractedDir, "webview", "assets");
  if (!fs.existsSync(webviewAssetsDir)) {
    console.warn(
      `WARN: Could not find webview assets directory in ${webviewAssetsDir} — skipping fast-mode model guard patch`,
    );
    return { changed: 0, matched: 0 };
  }

  const candidates = fs
    .readdirSync(webviewAssetsDir)
    .filter((name) => /\.js$/u.test(name))
    .sort();

  let changed = 0;
  let matched = 0;
  for (const candidate of candidates) {
    const filePath = path.join(webviewAssetsDir, candidate);
    try {
      const source = fs.readFileSync(filePath, "utf8");
      if (!hasFastModeModelGuardCandidate(source)) {
        continue;
      }
      matched += 1;
      const patched = applyLinuxFastModeModelGuardPatch(source);
      if (patched !== source) {
        fs.writeFileSync(filePath, patched, "utf8");
        changed += 1;
      }
    } catch (error) {
      console.warn(
        `WARN: Could not patch fast-mode model guard in ${filePath}: ${error.message}`,
      );
    }
  }
  if (matched === 0) {
    console.warn(
      `WARN: Could not find fast-mode model guard candidate in ${webviewAssetsDir} — skipping fast-mode model guard patch`,
    );
  }

  return { changed, matched };
}

module.exports = [
  {
    id: "linux-fast-mode-model-guard",
    phase: "extracted-app",
    order: 1040,
    ciPolicy: "required-upstream",
    apply: applyLinuxFastModeModelGuardPatchToExtractedApp,
  },
];
