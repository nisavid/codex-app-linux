"use strict";

const fs = require("node:fs");
const path = require("node:path");

const INTEGRATION_ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;
const APP_CONFIG_ID_PATTERN = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/;

function defaultPortIntegrationsRoot() {
  return path.resolve(__dirname, "..", "..", "port-integrations");
}

function portIntegrationsRoot(options = {}) {
  if (options.integrationsRoot != null) {
    return path.resolve(options.integrationsRoot);
  }
  if (options.featuresRoot != null) {
    return path.resolve(options.featuresRoot);
  }
  if (process.env.CODEX_PORT_INTEGRATIONS_ROOT?.trim()) {
    return path.resolve(process.env.CODEX_PORT_INTEGRATIONS_ROOT.trim());
  }
  if (process.env.CODEX_LINUX_FEATURES_ROOT?.trim()) {
    return path.resolve(process.env.CODEX_LINUX_FEATURES_ROOT.trim());
  }
  return defaultPortIntegrationsRoot();
}

function portIntegrationsConfigPath(integrationsRoot) {
  if (process.env.CODEX_PORT_INTEGRATIONS_CONFIG?.trim()) {
    return path.resolve(process.env.CODEX_PORT_INTEGRATIONS_CONFIG.trim());
  }
  if (process.env.CODEX_LINUX_FEATURES_CONFIG?.trim()) {
    return path.resolve(process.env.CODEX_LINUX_FEATURES_CONFIG.trim());
  }
  const localConfig = path.join(integrationsRoot, "integrations.json");
  if (fs.existsSync(localConfig)) {
    return localConfig;
  }
  const legacyLocalConfig = path.join(integrationsRoot, "features.json");
  if (fs.existsSync(legacyLocalConfig)) {
    return legacyLocalConfig;
  }
  const legacyCheckoutConfig = legacyCheckoutPortIntegrationsConfigPath(integrationsRoot);
  if (legacyCheckoutConfig != null && fs.existsSync(legacyCheckoutConfig)) {
    return legacyCheckoutConfig;
  }
  const userConfig = isCheckoutPortIntegrationsRoot(integrationsRoot) ? null : portIntegrationsUserConfigPath();
  if (userConfig != null && fs.existsSync(userConfig)) {
    return userConfig;
  }
  const legacyUserConfig = isCheckoutPortIntegrationsRoot(integrationsRoot) ? null : legacyPortIntegrationsUserConfigPath();
  if (legacyUserConfig != null && fs.existsSync(legacyUserConfig)) {
    return legacyUserConfig;
  }
  const legacyExampleConfig = path.join(integrationsRoot, "features.example.json");
  if (fs.existsSync(legacyExampleConfig)) {
    return legacyExampleConfig;
  }
  return path.join(integrationsRoot, "integrations.example.json");
}

function portIntegrationsConfigAppId() {
  for (const value of [process.env.CODEX_APP_ID, process.env.CODEX_LINUX_APP_ID]) {
    const configured = value?.trim();
    if (configured && APP_CONFIG_ID_PATTERN.test(configured)) {
      return configured;
    }
  }
  return "codex-app";
}

function isCheckoutPortIntegrationsRoot(integrationsRoot) {
  const resolvedRoot = path.resolve(integrationsRoot);
  if (!["port-integrations", "linux-features"].includes(path.basename(resolvedRoot))) {
    return false;
  }
  const repoRoot = path.dirname(resolvedRoot);
  return fs.existsSync(path.join(repoRoot, ".git"));
}

function legacyCheckoutPortIntegrationsConfigPath(integrationsRoot) {
  const resolvedRoot = path.resolve(integrationsRoot);
  if (path.basename(resolvedRoot) !== "port-integrations") {
    return null;
  }
  const repoRoot = path.dirname(resolvedRoot);
  const legacyRoot = path.join(repoRoot, "linux-features");
  return path.join(legacyRoot, "features.json");
}

function portIntegrationsUserConfigPath() {
  const xdgConfigHome = process.env.XDG_CONFIG_HOME?.trim();
  let configHome = null;
  if (xdgConfigHome && path.isAbsolute(xdgConfigHome)) {
    configHome = xdgConfigHome;
  } else if (process.env.HOME?.trim() && path.isAbsolute(process.env.HOME.trim())) {
    configHome = path.join(process.env.HOME.trim(), ".config");
  }
  if (configHome == null) {
    return null;
  }
  return path.join(configHome, portIntegrationsConfigAppId(), "port-integrations.json");
}

function legacyPortIntegrationsUserConfigPath() {
  const xdgConfigHome = process.env.XDG_CONFIG_HOME?.trim();
  let configHome = null;
  if (xdgConfigHome && path.isAbsolute(xdgConfigHome)) {
    configHome = xdgConfigHome;
  } else if (process.env.HOME?.trim() && path.isAbsolute(process.env.HOME.trim())) {
    configHome = path.join(process.env.HOME.trim(), ".config");
  }
  if (configHome == null) {
    return null;
  }
  return path.join(configHome, portIntegrationsConfigAppId(), "linux-features.json");
}

function readJsonFile(filePath, label) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    console.warn(`WARN: Could not read ${label} at ${filePath}: ${error.message}`);
    return null;
  }
}

function normalizeIntegrationIdList(value, sourcePath, key) {
  if (value == null) {
    return [];
  }
  if (!Array.isArray(value)) {
    console.warn(`WARN: port integrations config ${sourcePath} ${key} value must be an array`);
    return [];
  }

  const seen = new Set();
  const ids = [];
  for (const item of value) {
    if (typeof item !== "string" || !INTEGRATION_ID_PATTERN.test(item)) {
      console.warn(`WARN: Invalid port integration id in ${sourcePath} ${key}: ${String(item)}`);
      continue;
    }
    if (seen.has(item)) {
      continue;
    }
    seen.add(item);
    ids.push(item);
  }
  return ids;
}

function readPortIntegrationsConfig(integrationsRoot) {
  const configPath = portIntegrationsConfigPath(integrationsRoot);
  if (!fs.existsSync(configPath)) {
    return { enabled: [], disabled: [] };
  }

  const config = readJsonFile(configPath, "port integrations config");
  if (config == null) {
    return { enabled: [], disabled: [] };
  }
  return {
    enabled: normalizeIntegrationIdList(config.enabled, configPath, "enabled"),
    disabled: normalizeIntegrationIdList(config.disabled, configPath, "disabled"),
  };
}

function discoverPortIntegrationIds(integrationsRoot) {
  try {
    return fs.readdirSync(integrationsRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && INTEGRATION_ID_PATTERN.test(entry.name))
      .map((entry) => entry.name)
      .filter((id) => integrationManifestPath(integrationsRoot, id) != null)
      .sort();
  } catch (error) {
    console.warn(`WARN: Could not list port integrations at ${integrationsRoot}: ${error.message}`);
    return [];
  }
}

function enabledPortIntegrationIds(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  const config = readPortIntegrationsConfig(integrationsRoot);
  const disabled = new Set(config.disabled);
  const integrationsById = new Map();
  const seen = new Set();
  const ids = [];

  const integrationForId = (id) => {
    if (!integrationsById.has(id)) {
      integrationsById.set(id, loadPortIntegrationManifest(integrationsRoot, id));
    }
    return integrationsById.get(id);
  };

  const addIntegration = (id) => {
    if (disabled.has(id) || seen.has(id)) {
      return;
    }
    seen.add(id);
    ids.push(id);
  };

  for (const id of discoverPortIntegrationIds(integrationsRoot)) {
    const integration = integrationForId(id);
    if (integration?.manifest.defaultEnabled === true) {
      addIntegration(id);
    }
  }
  for (const id of config.enabled) {
    if (integrationForId(id) != null) {
      addIntegration(id);
    }
  }
  return ids;
}

function resolvedPortIntegrationsConfig(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  const knownIntegrationIds = new Set(discoverPortIntegrationIds(integrationsRoot));
  const config = readPortIntegrationsConfig(integrationsRoot);
  const enabled = enabledPortIntegrationIds({ ...options, integrationsRoot });
  const disabled = config.disabled.filter((id) => knownIntegrationIds.has(id));
  return { enabled, disabled };
}

function loadPortIntegrationManifest(integrationsRoot, id) {
  const integrationDir = path.join(integrationsRoot, id);
  const manifestPath = integrationManifestPath(integrationsRoot, id);
  if (manifestPath == null) {
    console.warn(`WARN: port integration '${id}' does not have integration.json`);
    return null;
  }

  const manifest = readJsonFile(manifestPath, `port integration '${id}' manifest`);
  if (manifest == null) {
    return null;
  }
  if (manifest.id !== id) {
    console.warn(`WARN: port integration '${id}' manifest id mismatch: ${String(manifest.id)}`);
    return null;
  }

  return { id, dir: integrationDir, manifestPath, manifest };
}

function integrationManifestPath(integrationsRoot, id) {
  const integrationDir = path.join(integrationsRoot, id);
  const manifestPath = path.join(integrationDir, "integration.json");
  if (fs.existsSync(manifestPath)) {
    return manifestPath;
  }
  const legacyManifestPath = path.join(integrationDir, "feature.json");
  if (fs.existsSync(legacyManifestPath)) {
    return legacyManifestPath;
  }
  return null;
}

function loadEnabledPortIntegrations(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  return enabledPortIntegrationIds({ ...options, integrationsRoot })
    .map((id) => loadPortIntegrationManifest(integrationsRoot, id))
    .filter(Boolean);
}

function resolveIntegrationEntrypoint(integration, key) {
  const relativePath = integration.manifest.entrypoints?.[key];
  if (relativePath == null) {
    return null;
  }
  if (typeof relativePath !== "string" || relativePath.trim().length === 0) {
    console.warn(`WARN: port integration '${integration.id}' has invalid ${key} entrypoint`);
    return null;
  }
  if (path.isAbsolute(relativePath)) {
    console.warn(`WARN: port integration '${integration.id}' ${key} entrypoint must stay inside the integration directory`);
    return null;
  }
  const entrypoint = path.resolve(integration.dir, relativePath);
  if (!fs.existsSync(entrypoint)) {
    console.warn(`WARN: port integration '${integration.id}' ${key} entrypoint not found: ${entrypoint}`);
    return null;
  }
  let realIntegrationDir;
  let realEntrypoint;
  try {
    realIntegrationDir = fs.realpathSync.native(integration.dir);
    realEntrypoint = fs.realpathSync.native(entrypoint);
  } catch (error) {
    console.warn(`WARN: Could not resolve port integration '${integration.id}' ${key}: ${error.message}`);
    return null;
  }
  const relativeEntrypoint = path.relative(realIntegrationDir, realEntrypoint);
  if (relativeEntrypoint === "" || relativeEntrypoint.startsWith("..") || path.isAbsolute(relativeEntrypoint)) {
    console.warn(`WARN: port integration '${integration.id}' ${key} entrypoint must stay inside the integration directory`);
    return null;
  }
  return realEntrypoint;
}

function loadIntegrationEntrypointModule(integration, key) {
  const entrypoint = resolveIntegrationEntrypoint(integration, key);
  if (entrypoint == null) {
    return null;
  }

  try {
    return {
      entrypoint,
      moduleExports: require(entrypoint),
    };
  } catch (error) {
    console.warn(`WARN: Could not load port integration '${integration.id}' ${key}: ${error.message}`);
    return null;
  }
}

function integrationContext(context, integration) {
  return { ...context, integration };
}

function prefixedIntegrationPatchId(integration, descriptorId) {
  return descriptorId.startsWith(`integration:${integration.id}`)
    ? descriptorId
    : `integration:${integration.id}:${descriptorId}`;
}

function wrapIntegrationPatchDescriptor(integration, descriptor, sourcePath, index, integrationIndex) {
  if (descriptor == null || typeof descriptor !== "object") {
    console.warn(`WARN: port integration '${integration.id}' patch descriptor ${index + 1} must be an object`);
    return null;
  }
  if (typeof descriptor.apply !== "function") {
    console.warn(`WARN: port integration '${integration.id}' patch descriptor ${index + 1} must export apply`);
    return null;
  }

  const descriptorId = descriptor.id ?? descriptor.name;
  if (typeof descriptorId !== "string" || descriptorId.length === 0) {
    console.warn(`WARN: port integration '${integration.id}' patch descriptor ${index + 1} must have id or name`);
    return null;
  }

  const wrappedId = prefixedIntegrationPatchId(integration, descriptorId);
  const wrapped = {
    ...descriptor,
    id: wrappedId,
    name: descriptor.name ?? wrappedId,
    ciPolicy: descriptor.ciPolicy ?? "optional",
    order: descriptor.order ?? 20_000 + integrationIndex * 100 + index * 10,
    sourcePath,
    apply: (target, context) => descriptor.apply(target, integrationContext(context, integration)),
  };

  if (typeof descriptor.appliesTo === "function") {
    wrapped.appliesTo = (context) => descriptor.appliesTo(integrationContext(context, integration));
  }
  if (typeof descriptor.enabled === "function") {
    wrapped.enabled = (context) => descriptor.enabled(integrationContext(context, integration));
  }
  if (typeof descriptor.targetSummary === "function") {
    wrapped.targetSummary = (context) => descriptor.targetSummary(integrationContext(context, integration));
  }
  if (typeof descriptor.status === "function") {
    wrapped.status = (result, warnings, context) =>
      descriptor.status(result, warnings, integrationContext(context, integration));
  }

  return wrapped;
}

function integrationPatchDescriptorListFromExports(integration, moduleExports, sourcePath, integrationIndex) {
  const exported = moduleExports?.descriptors ??
    moduleExports?.patches ??
    moduleExports?.default ??
    moduleExports;
  if (exported == null) {
    console.warn(`WARN: port integration '${integration.id}' patchDescriptors entrypoint must export descriptors`);
    return [];
  }

  const descriptors = Array.isArray(exported) ? exported : [exported];
  return descriptors
    .map((descriptor, index) =>
      wrapIntegrationPatchDescriptor(integration, descriptor, sourcePath, index, integrationIndex),
    )
    .filter(Boolean);
}

function loadPortIntegrationPatchDescriptors(options = {}) {
  const descriptors = [];
  for (const [integrationIndex, integration] of loadEnabledPortIntegrations(options).entries()) {
    const loaded = loadIntegrationEntrypointModule(integration, "patchDescriptors") ??
      loadIntegrationEntrypointModule(integration, "patches");
    if (loaded == null) {
      const legacyLoaded = loadIntegrationEntrypointModule(integration, "mainBundlePatch");
      if (legacyLoaded == null) {
        continue;
      }

      const moduleExports = legacyLoaded.moduleExports;
      const apply = moduleExports.applyMainBundlePatch ?? moduleExports.apply ?? moduleExports;
      if (typeof apply !== "function") {
        console.warn(`WARN: port integration '${integration.id}' mainBundlePatch must export a function`);
        continue;
      }

      descriptors.push({
        id: `integration:${integration.id}`,
        name: `integration:${integration.id}`,
        phase: "main-bundle",
        ciPolicy: "optional",
        apply: (source, context) => apply(source, integrationContext(context, integration)),
      });
      continue;
    }
    descriptors.push(
      ...integrationPatchDescriptorListFromExports(
        integration,
        loaded.moduleExports,
        loaded.entrypoint,
        integrationIndex,
      ),
    );
  }
  return descriptors;
}

function loadPortIntegrationMainBundlePatches(options = {}) {
  return loadPortIntegrationPatchDescriptors(options)
    .filter((patch) => (patch.phase ?? "main-bundle") === "main-bundle")
    .map(({ apply, ciPolicy, id, name }) => ({ apply, ciPolicy, id, name }));
}

function enabledPortIntegrationStageHooks(options = {}) {
  return loadEnabledPortIntegrations(options)
    .map((integration) => ({
      id: integration.id,
      path: resolveIntegrationEntrypoint(integration, "stageHook"),
    }))
    .filter((hook) => hook.path != null);
}

function main() {
  const command = process.argv[2];
  if (command === "--stage-hooks") {
    for (const hook of enabledPortIntegrationStageHooks()) {
      process.stdout.write(`${hook.id}\t${hook.path}\n`);
    }
    return;
  }
  if (command === "--enabled") {
    for (const id of enabledPortIntegrationIds()) {
      process.stdout.write(`${id}\n`);
    }
    return;
  }
  console.error("Usage: port-integrations.js --enabled | --stage-hooks");
  process.exit(1);
}

if (require.main === module) {
  main();
}

module.exports = {
  enabledPortIntegrationIds,
  enabledPortIntegrationStageHooks,
  loadEnabledPortIntegrations,
  loadPortIntegrationPatchDescriptors,
  loadPortIntegrationMainBundlePatches,
  portIntegrationsConfigPath,
  portIntegrationsRoot,
  portIntegrationsUserConfigPath,
  resolvedPortIntegrationsConfig,
  resolveIntegrationEntrypoint,
};
