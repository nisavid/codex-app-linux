"use strict";

const fs = require("node:fs");
const path = require("node:path");

const INTEGRATION_ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;
const APP_CONFIG_ID_PATTERN = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/;
const LOCAL_INTEGRATIONS_DIR = "local";
const RESERVED_TOP_LEVEL_NAMES = new Set([
  LOCAL_INTEGRATIONS_DIR,
  "README.md",
  "integrations.example.json",
  "integrations.json",
  "features.example.json",
  "features.json",
]);

const RUNTIME_HOOK_DIRS = {
  env: { dir: "env.d", executable: false },
  prelaunch: { dir: "prelaunch.d", executable: true },
  electronArgs: { dir: "electron-args.d", executable: false },
  coldStart: { dir: "cold-start.d", executable: true },
  afterExit: { dir: "after-exit.d", executable: true },
};
const STAGED_INTEGRATION_MANIFEST_RELATIVE_PATH = ".codex-linux/port-integrations-staged.json";

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

function portIntegrationsConfigPath(integrationsRoot, options = {}) {
  if (options.integrationsConfigPath != null) {
    return path.resolve(options.integrationsConfigPath);
  }
  if (options.featuresConfigPath != null) {
    return path.resolve(options.featuresConfigPath);
  }
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

function assertIntegrationId(value, label) {
  if (typeof value !== "string" || !INTEGRATION_ID_PATTERN.test(value)) {
    throw new Error(`${label} must match ${INTEGRATION_ID_PATTERN}`);
  }
  return value;
}

function normalizeIntegrationIdList(value, label, integrationId) {
  if (value == null) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new Error(`port integration '${integrationId}' ${label} must be an array`);
  }
  const seen = new Set();
  const result = [];
  for (const item of value) {
    assertIntegrationId(item, `port integration '${integrationId}' ${label} entry`);
    if (!seen.has(item)) {
      seen.add(item);
      result.push(item);
    }
  }
  return result;
}

function normalizeEnabledIntegrationIds(value, sourcePath) {
  return normalizeConfiguredIntegrationIdList(value, sourcePath, "enabled");
}

function normalizeConfiguredIntegrationIdList(value, sourcePath, key) {
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
      console.warn(`WARN: Invalid port integration id in ${sourcePath}: ${String(item)}`);
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

function readPortIntegrationsConfig(integrationsRoot, options = {}) {
  const configPath = portIntegrationsConfigPath(integrationsRoot, options);
  if (!fs.existsSync(configPath)) {
    return { enabled: [], disabled: [] };
  }

  const config = readJsonFile(configPath, "port integrations config");
  if (config == null) {
    return { enabled: [], disabled: [] };
  }
  return {
    enabled: normalizeConfiguredIntegrationIdList(config.enabled, configPath, "enabled"),
    disabled: normalizeConfiguredIntegrationIdList(config.disabled, configPath, "disabled"),
  };
}

function enabledPortIntegrationIds(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  const config = readPortIntegrationsConfig(integrationsRoot, options);
  const disabled = new Set(config.disabled);
  const integrationsById = portIntegrationManifestMap({ ...options, integrationsRoot });
  const seen = new Set();
  const ids = [];

  const addIntegration = (id) => {
    if (disabled.has(id) || seen.has(id)) {
      return;
    }
    if (!integrationsById.has(id)) {
      return;
    }
    seen.add(id);
    ids.push(id);
  };

  for (const integration of integrationsById.values()) {
    if (integration.manifest.defaultEnabled === true) {
      addIntegration(integration.id);
    }
  }
  for (const id of config.enabled) {
    addIntegration(id);
  }
  return ids;
}

function resolvedPortIntegrationsConfig(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  const knownIntegrationIds = new Set(discoverPortIntegrationManifests({ ...options, integrationsRoot }).map((integration) => integration.id));
  const config = readPortIntegrationsConfig(integrationsRoot, options);
  const enabled = enabledPortIntegrationIds({ ...options, integrationsRoot });
  const disabled = config.disabled.filter((id) => knownIntegrationIds.has(id));
  return { enabled, disabled };
}

function lstatPath(filePath) {
  try {
    return fs.lstatSync(filePath);
  } catch {
    return null;
  }
}

function isDirectory(filePath) {
  return lstatPath(filePath)?.isDirectory() === true;
}

function integrationManifestCandidates(integrationsRoot) {
  if (!fs.existsSync(integrationsRoot)) {
    return [];
  }

  const candidates = [];
  for (const name of fs.readdirSync(integrationsRoot).sort()) {
    if (RESERVED_TOP_LEVEL_NAMES.has(name) || name.startsWith(".")) {
      continue;
    }
    const dir = path.join(integrationsRoot, name);
    const manifestPath = integrationManifestPath(dir);
    if (isDirectory(dir) && manifestPath != null) {
      candidates.push({ dir, manifestPath, origin: "repo" });
    }
  }

  const localRoot = path.join(integrationsRoot, LOCAL_INTEGRATIONS_DIR);
  if (isDirectory(localRoot)) {
    for (const name of fs.readdirSync(localRoot).sort()) {
      if (name.startsWith(".")) {
        continue;
      }
      const dir = path.join(localRoot, name);
      const manifestPath = integrationManifestPath(dir);
      if (isDirectory(dir) && manifestPath != null) {
        candidates.push({ dir, manifestPath, origin: "local" });
      }
    }
  }

  return candidates;
}

function normalizePortIntegrationManifest(integrationsRoot, candidate) {
  const manifest = readJsonFile(candidate.manifestPath, "port integration manifest");
  if (manifest == null || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new Error(`port integration manifest ${candidate.manifestPath} must be a JSON object`);
  }

  const id = assertIntegrationId(manifest.id, `port integration id in ${candidate.manifestPath}`);
  const readmePath = path.join(candidate.dir, "README.md");
  if (!fs.existsSync(readmePath) || isDirectory(readmePath)) {
    throw new Error(`port integration '${id}' must include README.md next to integration.json`);
  }

  const relativeDir = path.relative(integrationsRoot, candidate.dir);
  return {
    id,
    dir: candidate.dir,
    manifestPath: candidate.manifestPath,
    readmePath,
    origin: candidate.origin,
    local: candidate.origin === "local",
    relativeDir,
    manifest: {
      ...manifest,
      defaultEnabled: manifest.defaultEnabled === true,
      requires: normalizeIntegrationIdList(manifest.requires, "requires", id),
      conflicts: normalizeIntegrationIdList(manifest.conflicts, "conflicts", id),
    },
  };
}

function integrationManifestPath(integrationDir) {
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

function discoverPortIntegrationManifests(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  const integrations = [];
  const seen = new Map();
  for (const candidate of integrationManifestCandidates(integrationsRoot)) {
    const integration = normalizePortIntegrationManifest(integrationsRoot, candidate);
    const previous = seen.get(integration.id);
    if (previous != null) {
      throw new Error(
        `Duplicate port integration id '${integration.id}' in ${integration.manifestPath} and ${previous.manifestPath}`,
      );
    }
    seen.set(integration.id, integration);
    integrations.push(integration);
  }
  return integrations.sort((left, right) => left.id.localeCompare(right.id));
}

function portIntegrationManifestMap(options = {}) {
  return new Map(discoverPortIntegrationManifests(options).map((integration) => [integration.id, integration]));
}

function loadPortIntegrationManifest(integrationsRoot, id, options = {}) {
  const integration = portIntegrationManifestMap({ ...options, integrationsRoot }).get(id);
  if (integration == null) {
    console.warn(`WARN: Enabled port integration '${id}' does not have integration.json`);
    return null;
  }
  return integration;
}

function validateEnabledIntegrationDependencies(integrations) {
  const enabled = new Set(integrations.map((integration) => integration.id));
  for (const integration of integrations) {
    for (const required of integration.manifest.requires) {
      if (!enabled.has(required)) {
        throw new Error(`port integration '${integration.id}' requires '${required}' to be enabled`);
      }
    }
    for (const conflict of integration.manifest.conflicts) {
      if (enabled.has(conflict)) {
        throw new Error(`port integration '${integration.id}' conflicts with '${conflict}'`);
      }
    }
  }
}

function loadEnabledPortIntegrations(options = {}) {
  const integrationsRoot = portIntegrationsRoot(options);
  const available = portIntegrationManifestMap({ ...options, integrationsRoot });
  const integrations = [];
  const missing = [];
  for (const id of enabledPortIntegrationIds({ ...options, integrationsRoot })) {
    const integration = available.get(id);
    if (integration == null) {
      missing.push(id);
    } else {
      integrations.push(integration);
    }
  }
  if (missing.length > 0) {
    throw new Error(`Enabled port integration ids not found in this checkout: ${missing.join(", ")}`);
  }
  validateEnabledIntegrationDependencies(integrations);
  return integrations;
}

function relativePathParts(relativePath) {
  return String(relativePath).split(/[\\/]+/).filter((part) => part.length > 0 && part !== ".");
}

function normalizeInstallRelativePath(relativePath, label) {
  if (typeof relativePath !== "string" || relativePath.trim().length === 0) {
    throw new Error(`${label} must be a relative path`);
  }
  const parts = relativePathParts(relativePath);
  if (path.isAbsolute(relativePath) || parts.includes("..")) {
    throw new Error(`${label} must stay inside the install directory`);
  }
  if (parts.length === 0) {
    throw new Error(`${label} must not target the install directory root`);
  }
  return parts.join("/");
}

function resolveInstallRelativePath(installDir, relativePath, label) {
  const normalized = normalizeInstallRelativePath(relativePath, label);
  const resolved = path.resolve(installDir, normalized);
  const relative = path.relative(installDir, resolved);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`${label} must stay inside the install directory`);
  }
  return { normalized, resolved };
}

function resolveIntegrationRelativePath(integration, relativePath, label, { mustExist = true } = {}) {
  if (typeof relativePath !== "string" || relativePath.trim().length === 0) {
    throw new Error(`port integration '${integration.id}' has invalid ${label}`);
  }
  if (path.isAbsolute(relativePath) || relativePathParts(relativePath).includes("..")) {
    throw new Error(`port integration '${integration.id}' ${label} must stay inside the integration directory`);
  }
  const resolved = path.resolve(integration.dir, relativePath);
  const relative = path.relative(integration.dir, resolved);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`port integration '${integration.id}' ${label} must stay inside the integration directory`);
  }
  if (mustExist && !fs.existsSync(resolved)) {
    throw new Error(`port integration '${integration.id}' ${label} not found: ${resolved}`);
  }
  return resolved;
}

function resolveIntegrationEntrypoint(integration, key) {
  const relativePath = integration.manifest.entrypoints?.[key];
  if (relativePath == null) {
    return null;
  }
  try {
    return resolveIntegrationRelativePath(integration, relativePath, `${key} entrypoint`);
  } catch (error) {
    console.warn(`WARN: ${error.message}`);
    return null;
  }
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
    sourceKind: descriptor.sourceKind ?? "integration",
    integrationId: integration.id,
    featureId: descriptor.featureId ?? integration.id,
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
        sourceKind: "integration",
        integrationId: integration.id,
        featureId: integration.id,
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

function normalizeEntryList(value, label, integration) {
  if (value == null) {
    return [];
  }
  const entries = Array.isArray(value) ? value : [value];
  return entries.map((entry, index) => {
    if (typeof entry === "string") {
      return { source: resolveIntegrationRelativePath(integration, entry, `${label} ${index + 1}`) };
    }
    if (entry == null || typeof entry !== "object" || Array.isArray(entry)) {
      throw new Error(`port integration '${integration.id}' ${label} ${index + 1} must be a string or object`);
    }
    const source = resolveIntegrationRelativePath(integration, entry.source ?? entry.path, `${label} ${index + 1}`);
    const name = entry.name == null ? path.basename(source) : String(entry.name);
    if (name.length === 0 || path.isAbsolute(name) || relativePathParts(name).includes("..") || name.includes("/") || name.includes("\\")) {
      throw new Error(`port integration '${integration.id}' ${label} ${index + 1} has invalid name`);
    }
    return { ...entry, source, name };
  });
}

function normalizeInstallTarget(target, integrationId) {
  return normalizeInstallRelativePath(target, `port integration '${integrationId}' resource target`);
}

function parseFileMode(value, fallback) {
  if (value == null) {
    return fallback;
  }
  if (typeof value !== "string") {
    throw new Error(`Invalid file mode: ${String(value)}; file mode must be a quoted octal string`);
  }
  const raw = value.trim();
  if (!/^[0-7]{3,4}$/.test(raw)) {
    throw new Error(`Invalid file mode: ${String(value)}; file mode must be a quoted octal string`);
  }
  return Number.parseInt(raw, 8);
}

function modeString(mode) {
  return mode == null ? null : mode.toString(8).padStart(4, "0");
}

function enabledPortIntegrationInstallPlan(options = {}) {
  const resources = [];
  const runtimeHooks = [];
  for (const integration of loadEnabledPortIntegrations(options)) {
    for (const [index, resource] of normalizeEntryList(integration.manifest.resources, "resource", integration).entries()) {
      const target = normalizeInstallTarget(resource.target, integration.id);
      resources.push({
        id: integration.id,
        source: resource.source,
        target,
        mode: resource.mode == null ? null : parseFileMode(resource.mode, 0o644),
        index,
      });
    }

    const hooks = integration.manifest.runtimeHooks ?? {};
    if (hooks != null && (typeof hooks !== "object" || Array.isArray(hooks))) {
      throw new Error(`port integration '${integration.id}' runtimeHooks must be an object`);
    }
    for (const [hookKey, hookSpec] of Object.entries(hooks ?? {})) {
      const runtimeHook = RUNTIME_HOOK_DIRS[hookKey];
      if (runtimeHook == null) {
        throw new Error(`port integration '${integration.id}' has unsupported runtime hook '${hookKey}'`);
      }
      for (const [index, entry] of normalizeEntryList(hookSpec, `runtimeHooks.${hookKey}`, integration).entries()) {
        const name = `${integration.id}-${entry.name ?? path.basename(entry.source)}`;
        runtimeHooks.push({
          id: integration.id,
          key: hookKey,
          source: entry.source,
          name,
          mode: parseFileMode(entry.mode, runtimeHook.executable ? 0o755 : 0o644),
          dir: runtimeHook.dir,
          target: [".codex-linux", runtimeHook.dir, name].join("/"),
          index,
        });
      }
    }
  }
  return { resources, runtimeHooks };
}

function chmodRecursive(target, mode) {
  const stats = lstatPath(target);
  if (stats == null || stats.isSymbolicLink()) {
    return;
  }
  const directory = stats.isDirectory();
  const targetMode = directory
    ? mode |
      ((mode & 0o400) ? 0o100 : 0) |
      ((mode & 0o040) ? 0o010 : 0) |
      ((mode & 0o004) ? 0o001 : 0)
    : mode;
  fs.chmodSync(target, targetMode);
  if (!directory) {
    return;
  }
  for (const name of fs.readdirSync(target)) {
    chmodRecursive(path.join(target, name), mode);
  }
}

function copyInstallFile(source, target, mode) {
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.cpSync(source, target, { recursive: true, force: true, dereference: false });
  if (mode != null) {
    chmodRecursive(target, mode);
  }
}

function stagedManifestPath(installDir) {
  return path.join(installDir, STAGED_INTEGRATION_MANIFEST_RELATIVE_PATH);
}

function stagedArtifactEntries(manifest) {
  if (manifest == null || typeof manifest !== "object" || Array.isArray(manifest)) {
    return [];
  }
  const resources = Array.isArray(manifest.resources) ? manifest.resources : [];
  const runtimeHooks = Array.isArray(manifest.runtimeHooks) ? manifest.runtimeHooks : [];
  return [...resources, ...runtimeHooks].filter((entry) => entry != null && typeof entry === "object");
}

function readStagedIntegrationManifest(installDir) {
  const manifestPath = stagedManifestPath(installDir);
  if (!fs.existsSync(manifestPath)) {
    return null;
  }
  try {
    return JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  } catch (error) {
    console.warn(`WARN: Could not read port integration staged manifest at ${manifestPath}: ${error.message}`);
    return null;
  }
}

function writeStagedIntegrationManifest(installDir, plan) {
  const manifestPath = stagedManifestPath(installDir);
  const manifest = {
    version: 1,
    resources: plan.resources.map((resource) => ({
      id: resource.id,
      type: "resource",
      target: resource.target,
      mode: modeString(resource.mode),
    })),
    runtimeHooks: plan.runtimeHooks.map((hook) => ({
      id: hook.id,
      type: "runtimeHook",
      key: hook.key,
      target: hook.target,
      mode: modeString(hook.mode),
    })),
  };
  fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

function removeInstallRelativePath(installDir, relativePath) {
  const { normalized, resolved } = resolveInstallRelativePath(
    installDir,
    relativePath,
    "port integration staged artifact target",
  );
  if (normalized === STAGED_INTEGRATION_MANIFEST_RELATIVE_PATH) {
    return;
  }
  fs.rmSync(resolved, { recursive: true, force: true });
}

function removePreviouslyStagedArtifacts(installDir, manifest) {
  for (const entry of stagedArtifactEntries(manifest)) {
    if (typeof entry.target !== "string") {
      continue;
    }
    removeInstallRelativePath(installDir, entry.target);
  }
}

function removeLegacyDeclarativeRuntimeHooks(installDir, options = {}) {
  const integrationIds = discoverPortIntegrationManifests(options).map((integration) => integration.id);
  if (integrationIds.length === 0) {
    return;
  }
  for (const runtimeHook of Object.values(RUNTIME_HOOK_DIRS)) {
    const hookDir = path.join(installDir, ".codex-linux", runtimeHook.dir);
    if (!isDirectory(hookDir)) {
      continue;
    }
    for (const name of fs.readdirSync(hookDir)) {
      if (integrationIds.some((id) => name.startsWith(`${id}-`))) {
        fs.rmSync(path.join(hookDir, name), { recursive: true, force: true });
      }
    }
  }
}

function stagedPortIntegrationFiles(appDir) {
  const installDir = path.resolve(appDir);
  return stagedArtifactEntries(readStagedIntegrationManifest(installDir))
    .filter((entry) => typeof entry.target === "string" && typeof entry.mode === "string")
    .map((entry) => ({
      id: entry.id ?? null,
      type: entry.type ?? null,
      key: entry.key ?? null,
      target: normalizeInstallRelativePath(entry.target, "port integration staged artifact target"),
      mode: entry.mode,
    }));
}

function stageEnabledPortIntegrationInstall(appDir, options = {}) {
  const installDir = path.resolve(appDir);
  const plan = enabledPortIntegrationInstallPlan(options);
  const previousManifest = readStagedIntegrationManifest(installDir);
  if (previousManifest == null) {
    removeLegacyDeclarativeRuntimeHooks(installDir, options);
  } else {
    removePreviouslyStagedArtifacts(installDir, previousManifest);
  }
  for (const resource of plan.resources) {
    copyInstallFile(resource.source, path.join(installDir, resource.target), resource.mode);
    console.error(`Staged port integration resource: ${resource.id} -> ${resource.target}`);
  }
  for (const hook of plan.runtimeHooks) {
    const target = path.join(installDir, hook.target);
    copyInstallFile(hook.source, target, hook.mode);
    console.error(`Staged port integration ${hook.key} hook: ${hook.id} -> ${path.relative(installDir, target)}`);
  }
  writeStagedIntegrationManifest(installDir, plan);
  return plan;
}

function enabledPortIntegrationPackageHooks(options = {}) {
  const packageFormat = options.packageFormat ?? null;
  const hooks = [];
  for (const integration of loadEnabledPortIntegrations(options)) {
    for (const [index, entry] of normalizeEntryList(integration.manifest.packageHooks, "packageHook", integration).entries()) {
      const formats = entry.formats == null
        ? []
        : normalizeIntegrationIdList(entry.formats, "packageHook formats", integration.id);
      if (packageFormat != null && formats.length > 0 && !formats.includes(packageFormat)) {
        continue;
      }
      hooks.push({
        id: integration.id,
        path: entry.source,
        formats,
        index,
      });
    }
  }
  return hooks;
}

function integrationsJsonSummary(options = {}) {
  return discoverPortIntegrationManifests(options).map((integration) => ({
    id: integration.id,
    title: integration.manifest.title ?? integration.manifest.name ?? integration.id,
    name: integration.manifest.name ?? integration.manifest.title ?? integration.id,
    description: integration.manifest.description ?? "",
    origin: integration.origin,
    local: integration.local,
    relativeDir: integration.relativeDir,
    requires: integration.manifest.requires,
    conflicts: integration.manifest.conflicts,
    defaultEnabled: integration.manifest.defaultEnabled === true,
    setup: integration.manifest.setup ?? null,
    cleanup: integration.manifest.cleanup ?? null,
  }));
}

function main() {
  const command = process.argv[2];
  if (command === "--stage-hooks") {
    for (const hook of enabledPortIntegrationStageHooks()) {
      process.stdout.write(`${hook.id}\t${hook.path}\n`);
    }
    return;
  }
  if (command === "--package-hooks") {
    const packageFormat = process.argv[3] ?? "";
    for (const hook of enabledPortIntegrationPackageHooks({ packageFormat })) {
      process.stdout.write(`${hook.id}\t${hook.path}\n`);
    }
    return;
  }
  if (command === "--stage-install") {
    const appDir = process.argv[3] ?? process.env.INSTALL_DIR;
    if (!appDir) {
      console.error("Usage: port-integrations.js --stage-install <install-dir>");
      process.exit(1);
    }
    stageEnabledPortIntegrationInstall(appDir);
    return;
  }
  if (command === "--staged-files-json") {
    const appDir = process.argv[3] ?? process.env.INSTALL_DIR;
    if (!appDir) {
      console.error("Usage: port-integrations.js --staged-files-json <install-dir>");
      process.exit(1);
    }
    process.stdout.write(`${JSON.stringify(stagedPortIntegrationFiles(appDir), null, 2)}\n`);
    return;
  }
  if (command === "--enabled") {
    for (const id of enabledPortIntegrationIds()) {
      process.stdout.write(`${id}\n`);
    }
    return;
  }
  if (command === "--integrations-json" || command === "--features-json") {
    process.stdout.write(`${JSON.stringify(integrationsJsonSummary(), null, 2)}\n`);
    return;
  }
  if (command === "--resolved-config-json") {
    process.stdout.write(`${JSON.stringify(resolvedPortIntegrationsConfig(), null, 2)}\n`);
    return;
  }
  if (command === "--integrations-root" || command === "--features-root") {
    process.stdout.write(`${portIntegrationsRoot()}\n`);
    return;
  }
  console.error("Usage: port-integrations.js --enabled | --integrations-json | --features-json | --resolved-config-json | --integrations-root | --features-root | --stage-install <install-dir> | --staged-files-json <install-dir> | --stage-hooks | --package-hooks <format>");
  process.exit(1);
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(`ERROR: ${error.message}`);
    process.exit(1);
  }
}

module.exports = {
  discoverPortIntegrationManifests,
  enabledPortIntegrationIds,
  enabledPortIntegrationInstallPlan,
  enabledPortIntegrationPackageHooks,
  enabledPortIntegrationStageHooks,
  integrationsJsonSummary,
  loadEnabledPortIntegrations,
  loadPortIntegrationPatchDescriptors,
  loadPortIntegrationMainBundlePatches,
  portIntegrationManifestMap,
  portIntegrationsConfigPath,
  portIntegrationsRoot,
  portIntegrationsUserConfigPath,
  resolvedPortIntegrationsConfig,
  resolveIntegrationEntrypoint,
  stageEnabledPortIntegrationInstall,
  stagedPortIntegrationFiles,
};
