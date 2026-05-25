"use strict";

function applyMainBundlePatch(source) {
  const marker = "codexLinuxExampleIntegrationDisabled()";
  if (!source.includes(marker)) {
    console.warn("WARN: Example port integration marker not found — skipping example integration patch");
    return source;
  }
  return source.replace(marker, "codexLinuxExampleIntegrationEnabled()");
}

module.exports = {
  applyMainBundlePatch,
};
