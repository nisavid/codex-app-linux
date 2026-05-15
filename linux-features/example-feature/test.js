#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { applyMainBundlePatch } = require("./patch.js");
const {
  enabledLinuxFeatureIds,
  enabledLinuxFeatureStageHooks,
  loadLinuxFeatureMainBundlePatches,
} = require("../../scripts/lib/linux-features.js");
const {
  createPatchReport,
  patchExtractedApp,
  patchMainBundleSource,
} = require("../../scripts/patch-linux-window-ui.js");

function withTempFeatureRoot(config, fn) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "codex-example-feature-test-"));
  try {
    fs.writeFileSync(path.join(root, "features.example.json"), JSON.stringify({ enabled: [], disabled: [] }, null, 2));
    const featureConfig = Array.isArray(config) ? { enabled: config } : config;
    if (featureConfig != null) {
      fs.writeFileSync(path.join(root, "features.json"), JSON.stringify(featureConfig, null, 2));
    }
    fs.cpSync(__dirname, path.join(root, "example-feature"), { recursive: true });
    return fn(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

test("example feature patches only its synthetic marker", () => {
  assert.equal(
    applyMainBundlePatch("before;codexLinuxExampleFeatureDisabled();after"),
    "before;codexLinuxExampleFeatureEnabled();after",
  );
});

test("example feature is a no-op without its synthetic marker", () => {
  const originalWarn = console.warn;
  console.warn = () => {};
  try {
    assert.equal(applyMainBundlePatch("real codex bundle"), "real codex bundle");
  } finally {
    console.warn = originalWarn;
  }
});

test("example feature stays disabled until listed in features.json", () => {
  withTempFeatureRoot([], (root) => {
    assert.deepEqual(enabledLinuxFeatureIds({ featuresRoot: root }), []);
    assert.deepEqual(enabledLinuxFeatureStageHooks({ featuresRoot: root }), []);
    assert.deepEqual(loadLinuxFeatureMainBundlePatches({ featuresRoot: root }), []);
  });
});

test("missing explicitly enabled features are not reported as enabled", () => {
  withTempFeatureRoot(["missing-feature"], (root) => {
    assert.deepEqual(enabledLinuxFeatureIds({ featuresRoot: root }), []);
  });
});

test("default-enabled features load unless listed in disabled", () => {
  withTempFeatureRoot([], (root) => {
    const manifestPath = path.join(root, "example-feature", "feature.json");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    manifest.defaultEnabled = true;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

    fs.writeFileSync(path.join(root, "features.json"), JSON.stringify({}, null, 2));
    assert.deepEqual(enabledLinuxFeatureIds({ featuresRoot: root }), ["example-feature"]);

    fs.writeFileSync(
      path.join(root, "features.json"),
      JSON.stringify({ disabled: ["example-feature"] }, null, 2),
    );
    assert.deepEqual(enabledLinuxFeatureIds({ featuresRoot: root }), []);
  });
});

test("default-enabled features can be disabled from XDG user config", () => {
  withTempFeatureRoot(null, (root) => {
    const manifestPath = path.join(root, "example-feature", "feature.json");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    manifest.defaultEnabled = true;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

    const configHome = path.join(root, "xdg-config");
    const appConfigDir = path.join(configHome, "codex-app");
    fs.mkdirSync(appConfigDir, { recursive: true });
    fs.writeFileSync(
      path.join(appConfigDir, "linux-features.json"),
      JSON.stringify({ disabled: ["example-feature"] }, null, 2),
    );

    const originalConfigHome = process.env.XDG_CONFIG_HOME;
    const originalHome = process.env.HOME;
    const originalConfig = process.env.CODEX_LINUX_FEATURES_CONFIG;
    const originalAppId = process.env.CODEX_APP_ID;
    const originalLinuxAppId = process.env.CODEX_LINUX_APP_ID;
    try {
      process.env.XDG_CONFIG_HOME = configHome;
      process.env.HOME = path.join(root, "home");
      delete process.env.CODEX_LINUX_FEATURES_CONFIG;
      delete process.env.CODEX_APP_ID;
      delete process.env.CODEX_LINUX_APP_ID;
      assert.deepEqual(enabledLinuxFeatureIds({ featuresRoot: root }), []);
    } finally {
      if (originalConfigHome == null) {
        delete process.env.XDG_CONFIG_HOME;
      } else {
        process.env.XDG_CONFIG_HOME = originalConfigHome;
      }
      if (originalHome == null) {
        delete process.env.HOME;
      } else {
        process.env.HOME = originalHome;
      }
      if (originalConfig == null) {
        delete process.env.CODEX_LINUX_FEATURES_CONFIG;
      } else {
        process.env.CODEX_LINUX_FEATURES_CONFIG = originalConfig;
      }
      if (originalAppId == null) {
        delete process.env.CODEX_APP_ID;
      } else {
        process.env.CODEX_APP_ID = originalAppId;
      }
      if (originalLinuxAppId == null) {
        delete process.env.CODEX_LINUX_APP_ID;
      } else {
        process.env.CODEX_LINUX_APP_ID = originalLinuxAppId;
      }
    }
  });
});

test("example feature exposes its patch and stage hook when enabled", () => {
  withTempFeatureRoot(["example-feature"], (root) => {
    assert.deepEqual(enabledLinuxFeatureIds({ featuresRoot: root }), ["example-feature"]);

    const hooks = enabledLinuxFeatureStageHooks({ featuresRoot: root });
    assert.equal(hooks.length, 1);
    assert.equal(hooks[0].id, "example-feature");
    assert.equal(path.basename(hooks[0].path), "stage.sh");

    const patches = loadLinuxFeatureMainBundlePatches({ featuresRoot: root });
    assert.equal(patches.length, 1);
    assert.equal(patches[0].name, "feature:example-feature");
    assert.equal(
      patches[0].apply("codexLinuxExampleFeatureDisabled()", {}),
      "codexLinuxExampleFeatureEnabled()",
    );
  });
});

test("example feature participates in main bundle patching and patch reports", () => {
  withTempFeatureRoot(["example-feature"], (root) => {
    const originalRoot = process.env.CODEX_LINUX_FEATURES_ROOT;
    process.env.CODEX_LINUX_FEATURES_ROOT = root;
    const tempApp = fs.mkdtempSync(path.join(os.tmpdir(), "codex-example-feature-app-"));
    try {
      assert.equal(
        patchMainBundleSource("codexLinuxExampleFeatureDisabled()", null),
        "codexLinuxExampleFeatureEnabled()",
      );

      const buildDir = path.join(tempApp, ".vite", "build");
      fs.mkdirSync(buildDir, { recursive: true });
      fs.writeFileSync(path.join(buildDir, "main.js"), "codexLinuxExampleFeatureDisabled()");

      const report = createPatchReport();
      patchExtractedApp(tempApp, { report });

      assert.match(fs.readFileSync(path.join(buildDir, "main.js"), "utf8"), /codexLinuxExampleFeatureEnabled\(\)/);
      assert.ok(report.patches.some((patch) => patch.name === "feature:example-feature" && patch.status === "applied"));
    } finally {
      if (originalRoot == null) {
        delete process.env.CODEX_LINUX_FEATURES_ROOT;
      } else {
        process.env.CODEX_LINUX_FEATURES_ROOT = originalRoot;
      }
      fs.rmSync(tempApp, { recursive: true, force: true });
    }
  });
});

test("example feature stage hook is runnable through the Linux feature shell runner", () => {
  withTempFeatureRoot(["example-feature"], (root) => {
    const marker = path.join(root, "stage-marker.txt");
    const repoRoot = path.resolve(__dirname, "..", "..");
    const runner = path.join(repoRoot, "scripts", "lib", "linux-features.sh");
    const result = spawnSync(
      "bash",
      [
        "-lc",
        [
          "source \"$LINUX_FEATURES_RUNNER\"",
          "info(){ echo \"$*\" >&2; }",
          "warn(){ echo \"$*\" >&2; }",
          "SCRIPT_DIR=\"$REPO_ROOT\"",
          "INSTALL_DIR=\"$TMP_INSTALL_DIR\"",
          "WORK_DIR=\"$TMP_WORK_DIR\"",
          "ARCH=x86_64",
          "run_linux_feature_stage_hooks",
        ].join("\n"),
      ],
      {
        env: {
          ...process.env,
          LINUX_FEATURES_RUNNER: runner,
          REPO_ROOT: repoRoot,
          TMP_INSTALL_DIR: path.join(root, "install"),
          TMP_WORK_DIR: path.join(root, "work"),
          CODEX_LINUX_FEATURES_ROOT: root,
          CODEX_EXAMPLE_FEATURE_STAGE_MARKER: marker,
        },
        encoding: "utf8",
      },
    );

    assert.equal(result.status, 0, result.stderr);
    assert.match(fs.readFileSync(marker, "utf8"), /example-stage:x86_64:/);
    assert.match(result.stderr, /Running Linux feature stage hook: example-feature/);
  });
});

test("Linux feature shell runner fails when an enabled stage hook fails", () => {
  withTempFeatureRoot(["example-feature"], (root) => {
    fs.writeFileSync(
      path.join(root, "example-feature", "stage.sh"),
      "#!/bin/bash\nset -Eeuo pipefail\nexit 42\n",
    );
    const repoRoot = path.resolve(__dirname, "..", "..");
    const runner = path.join(repoRoot, "scripts", "lib", "linux-features.sh");
    const result = spawnSync(
      "bash",
      [
        "-lc",
        [
          "source \"$LINUX_FEATURES_RUNNER\"",
          "info(){ echo \"$*\" >&2; }",
          "warn(){ echo \"$*\" >&2; }",
          "SCRIPT_DIR=\"$REPO_ROOT\"",
          "INSTALL_DIR=\"$TMP_INSTALL_DIR\"",
          "WORK_DIR=\"$TMP_WORK_DIR\"",
          "ARCH=x86_64",
          "run_linux_feature_stage_hooks",
        ].join("\n"),
      ],
      {
        env: {
          ...process.env,
          LINUX_FEATURES_RUNNER: runner,
          REPO_ROOT: repoRoot,
          TMP_INSTALL_DIR: path.join(root, "install"),
          TMP_WORK_DIR: path.join(root, "work"),
          CODEX_LINUX_FEATURES_ROOT: root,
        },
        encoding: "utf8",
      },
    );

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Linux feature stage hook failed: example-feature/);
  });
});
