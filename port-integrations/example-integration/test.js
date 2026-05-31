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
  enabledPortIntegrationIds,
  enabledPortIntegrationStageHooks,
  loadPortIntegrationMainBundlePatches,
  portIntegrationsConfigPath,
} = require("../../scripts/lib/port-integrations.js");
const {
  createPatchReport,
  patchExtractedApp,
  patchMainBundleSource,
} = require("../../scripts/patch-linux-window-ui.js");

function withTempIntegrationRoot(config, fn) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "codex-example-integration-test-"));
  try {
    fs.writeFileSync(path.join(root, "integrations.example.json"), JSON.stringify({ enabled: [], disabled: [] }, null, 2));
    const integrationConfig = Array.isArray(config) ? { enabled: config } : config;
    if (integrationConfig != null) {
      fs.writeFileSync(path.join(root, "integrations.json"), JSON.stringify(integrationConfig, null, 2));
    }
    fs.cpSync(__dirname, path.join(root, "example-integration"), { recursive: true });
    return fn(root);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function withTempCheckoutIntegrationRoot(fn) {
  const repo = fs.mkdtempSync(path.join(os.tmpdir(), "codex-example-integration-checkout-"));
  try {
    fs.writeFileSync(path.join(repo, ".git"), "gitdir: /tmp/fake-worktree\n");
    const root = path.join(repo, "port-integrations");
    fs.mkdirSync(root, { recursive: true });
    fs.writeFileSync(path.join(root, "integrations.example.json"), JSON.stringify({ enabled: [], disabled: [] }, null, 2));
    fs.cpSync(__dirname, path.join(root, "example-integration"), { recursive: true });
    return fn(root);
  } finally {
    fs.rmSync(repo, { recursive: true, force: true });
  }
}

test("example integration patches only its synthetic marker", () => {
  assert.equal(
    applyMainBundlePatch("before;codexLinuxExampleIntegrationDisabled();after"),
    "before;codexLinuxExampleIntegrationEnabled();after",
  );
});

test("example integration is a no-op without its synthetic marker", () => {
  const originalWarn = console.warn;
  console.warn = () => {};
  try {
    assert.equal(applyMainBundlePatch("real codex bundle"), "real codex bundle");
  } finally {
    console.warn = originalWarn;
  }
});

test("example integration stays disabled until listed in integrations.json", () => {
  withTempIntegrationRoot([], (root) => {
    assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), []);
    assert.deepEqual(enabledPortIntegrationStageHooks({ integrationsRoot: root }), []);
    assert.deepEqual(loadPortIntegrationMainBundlePatches({ integrationsRoot: root }), []);
  });
});

test("missing explicitly enabled integrations are not reported as enabled", () => {
  withTempIntegrationRoot(["missing-integration"], (root) => {
    assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), []);
  });
});

test("default-enabled integrations load unless listed in disabled", () => {
  withTempIntegrationRoot([], (root) => {
    const manifestPath = path.join(root, "example-integration", "integration.json");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    manifest.defaultEnabled = true;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

    fs.writeFileSync(path.join(root, "integrations.json"), JSON.stringify({}, null, 2));
    assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), ["example-integration"]);

    fs.writeFileSync(
      path.join(root, "integrations.json"),
      JSON.stringify({ disabled: ["example-integration"] }, null, 2),
    );
    assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), []);
  });
});

test("default-enabled integrations can be disabled from XDG user config", () => {
  withTempIntegrationRoot(null, (root) => {
    const manifestPath = path.join(root, "example-integration", "integration.json");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    manifest.defaultEnabled = true;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

    const configHome = path.join(root, "xdg-config");
    const appConfigDir = path.join(configHome, "codex-app");
    fs.mkdirSync(appConfigDir, { recursive: true });
    fs.writeFileSync(
      path.join(appConfigDir, "port-integrations.json"),
      JSON.stringify({ disabled: ["example-integration"] }, null, 2),
    );

    const originalConfigHome = process.env.XDG_CONFIG_HOME;
    const originalHome = process.env.HOME;
    const originalConfig = process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
    const originalAppId = process.env.CODEX_APP_ID;
    const originalLinuxAppId = process.env.CODEX_LINUX_APP_ID;
    try {
      process.env.XDG_CONFIG_HOME = configHome;
      process.env.HOME = path.join(root, "home");
      delete process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
      delete process.env.CODEX_APP_ID;
      delete process.env.CODEX_LINUX_APP_ID;
      assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), []);
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
        delete process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
      } else {
        process.env.CODEX_PORT_INTEGRATIONS_CONFIG = originalConfig;
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

test("checkout integration roots ignore persistent XDG user config fallback", () => {
  withTempCheckoutIntegrationRoot((root) => {
    const manifestPath = path.join(root, "example-integration", "integration.json");
    const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
    manifest.defaultEnabled = true;
    fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

    const configHome = path.join(root, "xdg-config");
    const appConfigDir = path.join(configHome, "codex-app");
    fs.mkdirSync(appConfigDir, { recursive: true });
    fs.writeFileSync(
      path.join(appConfigDir, "port-integrations.json"),
      JSON.stringify({ disabled: ["example-integration"] }, null, 2),
    );

    const originalConfigHome = process.env.XDG_CONFIG_HOME;
    const originalHome = process.env.HOME;
    const originalConfig = process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
    try {
      process.env.XDG_CONFIG_HOME = configHome;
      process.env.HOME = path.join(root, "home");
      delete process.env.CODEX_PORT_INTEGRATIONS_CONFIG;

      assert.equal(portIntegrationsConfigPath(root), path.join(root, "integrations.example.json"));
      assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), ["example-integration"]);
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
        delete process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
      } else {
        process.env.CODEX_PORT_INTEGRATIONS_CONFIG = originalConfig;
      }
    }
  });
});

test("legacy Linux feature option and manifest names remain compatibility aliases", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "codex-legacy-linux-features-"));
  try {
    const integrationDir = path.join(root, "example-integration");
    fs.mkdirSync(integrationDir, { recursive: true });
    fs.copyFileSync(path.join(__dirname, "integration.json"), path.join(integrationDir, "feature.json"));
    fs.copyFileSync(path.join(__dirname, "README.md"), path.join(integrationDir, "README.md"));
    fs.writeFileSync(
      path.join(root, "features.json"),
      JSON.stringify({ enabled: ["example-integration"] }, null, 2),
    );

    assert.equal(portIntegrationsConfigPath(root), path.join(root, "features.json"));
    assert.deepEqual(enabledPortIntegrationIds({ featuresRoot: root }), ["example-integration"]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("empty CODEX_APP_ID does not block CODEX_LINUX_APP_ID config fallback", () => {
  withTempIntegrationRoot(null, (root) => {
    const configHome = path.join(root, "xdg-config");
    const appConfigDir = path.join(configHome, "codex-cua-lab");
    const configPath = path.join(appConfigDir, "port-integrations.json");
    fs.mkdirSync(appConfigDir, { recursive: true });
    fs.writeFileSync(configPath, JSON.stringify({ enabled: [] }, null, 2));

    const originalConfigHome = process.env.XDG_CONFIG_HOME;
    const originalHome = process.env.HOME;
    const originalConfig = process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
    const originalAppId = process.env.CODEX_APP_ID;
    const originalLinuxAppId = process.env.CODEX_LINUX_APP_ID;
    try {
      process.env.XDG_CONFIG_HOME = configHome;
      process.env.HOME = path.join(root, "home");
      delete process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
      process.env.CODEX_APP_ID = "  ";
      process.env.CODEX_LINUX_APP_ID = "codex-cua-lab";

      assert.equal(portIntegrationsConfigPath(root), configPath);
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
        delete process.env.CODEX_PORT_INTEGRATIONS_CONFIG;
      } else {
        process.env.CODEX_PORT_INTEGRATIONS_CONFIG = originalConfig;
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

test("example integration exposes its patch and stage hook when enabled", () => {
  withTempIntegrationRoot(["example-integration"], (root) => {
    assert.deepEqual(enabledPortIntegrationIds({ integrationsRoot: root }), ["example-integration"]);

    const hooks = enabledPortIntegrationStageHooks({ integrationsRoot: root });
    assert.equal(hooks.length, 1);
    assert.equal(hooks[0].id, "example-integration");
    assert.equal(path.basename(hooks[0].path), "stage.sh");

    const patches = loadPortIntegrationMainBundlePatches({ integrationsRoot: root });
    assert.equal(patches.length, 1);
    assert.equal(patches[0].name, "integration:example-integration");
    assert.equal(
      patches[0].apply("codexLinuxExampleIntegrationDisabled()", {}),
      "codexLinuxExampleIntegrationEnabled()",
    );
  });
});

test("example integration participates in main bundle patching and patch reports", () => {
  withTempIntegrationRoot(["example-integration"], (root) => {
    const originalRoot = process.env.CODEX_PORT_INTEGRATIONS_ROOT;
    process.env.CODEX_PORT_INTEGRATIONS_ROOT = root;
    const tempApp = fs.mkdtempSync(path.join(os.tmpdir(), "codex-example-integration-app-"));
    try {
      assert.equal(
        patchMainBundleSource("codexLinuxExampleIntegrationDisabled()", null),
        "codexLinuxExampleIntegrationEnabled()",
      );

      const buildDir = path.join(tempApp, ".vite", "build");
      fs.mkdirSync(buildDir, { recursive: true });
      fs.writeFileSync(path.join(buildDir, "main.js"), "codexLinuxExampleIntegrationDisabled()");

      const report = createPatchReport();
      patchExtractedApp(tempApp, { report });

      assert.match(fs.readFileSync(path.join(buildDir, "main.js"), "utf8"), /codexLinuxExampleIntegrationEnabled\(\)/);
      assert.ok(report.patches.some((patch) => patch.name === "integration:example-integration" && patch.status === "applied"));
    } finally {
      if (originalRoot == null) {
        delete process.env.CODEX_PORT_INTEGRATIONS_ROOT;
      } else {
        process.env.CODEX_PORT_INTEGRATIONS_ROOT = originalRoot;
      }
      fs.rmSync(tempApp, { recursive: true, force: true });
    }
  });
});

test("example integration stage hook is runnable through the port integration shell runner", () => {
  withTempIntegrationRoot(["example-integration"], (root) => {
    const marker = path.join(root, "stage-marker.txt");
    const repoRoot = path.resolve(__dirname, "..", "..");
    const runner = path.join(repoRoot, "scripts", "lib", "port-integrations.sh");
    const result = spawnSync(
      "bash",
      [
        "-lc",
        [
          "source \"$PORT_INTEGRATIONS_RUNNER\"",
          "info(){ echo \"$*\" >&2; }",
          "warn(){ echo \"$*\" >&2; }",
          "SCRIPT_DIR=\"$REPO_ROOT\"",
          "INSTALL_DIR=\"$TMP_INSTALL_DIR\"",
          "WORK_DIR=\"$TMP_WORK_DIR\"",
          "ARCH=x86_64",
          "run_port_integration_stage_hooks",
        ].join("\n"),
      ],
      {
        env: {
          ...process.env,
          PORT_INTEGRATIONS_RUNNER: runner,
          REPO_ROOT: repoRoot,
          TMP_INSTALL_DIR: path.join(root, "install"),
          TMP_WORK_DIR: path.join(root, "work"),
          CODEX_PORT_INTEGRATIONS_ROOT: root,
          CODEX_EXAMPLE_INTEGRATION_STAGE_MARKER: marker,
        },
        encoding: "utf8",
      },
    );

    assert.equal(result.status, 0, result.stderr);
    assert.match(fs.readFileSync(marker, "utf8"), /example-stage:x86_64:/);
    assert.match(result.stderr, /Running port integration stage hook: example-integration/);
  });
});

test("port integration shell runner fails when an enabled stage hook fails", () => {
  withTempIntegrationRoot(["example-integration"], (root) => {
    fs.writeFileSync(
      path.join(root, "example-integration", "stage.sh"),
      "#!/bin/bash\nset -Eeuo pipefail\nexit 42\n",
    );
    const repoRoot = path.resolve(__dirname, "..", "..");
    const runner = path.join(repoRoot, "scripts", "lib", "port-integrations.sh");
    const result = spawnSync(
      "bash",
      [
        "-lc",
        [
          "source \"$PORT_INTEGRATIONS_RUNNER\"",
          "info(){ echo \"$*\" >&2; }",
          "warn(){ echo \"$*\" >&2; }",
          "SCRIPT_DIR=\"$REPO_ROOT\"",
          "INSTALL_DIR=\"$TMP_INSTALL_DIR\"",
          "WORK_DIR=\"$TMP_WORK_DIR\"",
          "ARCH=x86_64",
          "run_port_integration_stage_hooks",
        ].join("\n"),
      ],
      {
        env: {
          ...process.env,
          PORT_INTEGRATIONS_RUNNER: runner,
          REPO_ROOT: repoRoot,
          TMP_INSTALL_DIR: path.join(root, "install"),
          TMP_WORK_DIR: path.join(root, "work"),
          CODEX_PORT_INTEGRATIONS_ROOT: root,
        },
        encoding: "utf8",
      },
    );

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /port integration stage hook failed: example-integration/);
  });
});
