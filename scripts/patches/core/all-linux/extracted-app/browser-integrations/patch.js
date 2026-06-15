"use strict";

const { patchLinuxChromeNativeHostRuntimeAssets } = require("../../../../chrome-plugin.js");

module.exports = [
  {
    id: "linux-chrome-native-host-runtime",
    phase: "extracted-app",
    order: 180,
    ciPolicy: "optional",
    apply: patchLinuxChromeNativeHostRuntimeAssets,
	    status: (result, warnings) => ({
	      status: result?.changed
	        ? warnings.length > 0 && result?.matched
	          ? "skipped-optional"
	          : "applied"
	        : result?.matched
          ? warnings.length > 0
            ? "skipped-optional"
            : "already-applied"
          : "skipped-optional",
      reason:
        result?.reason ??
        warnings[0] ??
        (result?.matched ? null : "Chrome native host runtime resolver not found"),
    }),
  },
];
