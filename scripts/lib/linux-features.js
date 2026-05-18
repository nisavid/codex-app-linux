"use strict";

const fs = require("node:fs");
const path = require("node:path");

const FEATURE_ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;
const APP_CONFIG_ID_PATTERN = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/;

function defaultLinuxFeaturesRoot() {
  return path.resolve(__dirname, "..", "..", "linux-features");
}

function linuxFeaturesRoot(options = {}) {
  if (options.featuresRoot != null) {
    return path.resolve(options.featuresRoot);
  }
  if (process.env.CODEX_LINUX_FEATURES_ROOT?.trim()) {
    return path.resolve(process.env.CODEX_LINUX_FEATURES_ROOT.trim());
  }
  return defaultLinuxFeaturesRoot();
}

function linuxFeaturesConfigPath(featuresRoot) {
  if (process.env.CODEX_LINUX_FEATURES_CONFIG?.trim()) {
    return path.resolve(process.env.CODEX_LINUX_FEATURES_CONFIG.trim());
  }
  const localConfig = path.join(featuresRoot, "features.json");
  if (fs.existsSync(localConfig)) {
    return localConfig;
  }
  const userConfig = isCheckoutLinuxFeaturesRoot(featuresRoot) ? null : linuxFeaturesUserConfigPath();
  if (userConfig != null && fs.existsSync(userConfig)) {
    return userConfig;
  }
  return path.join(featuresRoot, "features.example.json");
}

function linuxFeaturesConfigAppId() {
  for (const value of [process.env.CODEX_APP_ID, process.env.CODEX_LINUX_APP_ID]) {
    const configured = value?.trim();
    if (configured && APP_CONFIG_ID_PATTERN.test(configured)) {
      return configured;
    }
  }
  return "codex-app";
}

function isCheckoutLinuxFeaturesRoot(featuresRoot) {
  const resolvedRoot = path.resolve(featuresRoot);
  if (path.basename(resolvedRoot) !== "linux-features") {
    return false;
  }
  const repoRoot = path.dirname(resolvedRoot);
  return fs.existsSync(path.join(repoRoot, ".git"));
}

function linuxFeaturesUserConfigPath() {
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
  return path.join(configHome, linuxFeaturesConfigAppId(), "linux-features.json");
}

function readJsonFile(filePath, label) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    console.warn(`WARN: Could not read ${label} at ${filePath}: ${error.message}`);
    return null;
  }
}

function normalizeFeatureIdList(value, sourcePath, key) {
  if (value == null) {
    return [];
  }
  if (!Array.isArray(value)) {
    console.warn(`WARN: Linux features config ${sourcePath} ${key} value must be an array`);
    return [];
  }

  const seen = new Set();
  const ids = [];
  for (const item of value) {
    if (typeof item !== "string" || !FEATURE_ID_PATTERN.test(item)) {
      console.warn(`WARN: Invalid Linux feature id in ${sourcePath} ${key}: ${String(item)}`);
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

function readLinuxFeaturesConfig(featuresRoot) {
  const configPath = linuxFeaturesConfigPath(featuresRoot);
  if (!fs.existsSync(configPath)) {
    return { enabled: [], disabled: [] };
  }

  const config = readJsonFile(configPath, "Linux features config");
  if (config == null) {
    return { enabled: [], disabled: [] };
  }
  return {
    enabled: normalizeFeatureIdList(config.enabled, configPath, "enabled"),
    disabled: normalizeFeatureIdList(config.disabled, configPath, "disabled"),
  };
}

function discoverLinuxFeatureIds(featuresRoot) {
  try {
    return fs.readdirSync(featuresRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory() && FEATURE_ID_PATTERN.test(entry.name))
      .map((entry) => entry.name)
      .filter((id) => fs.existsSync(path.join(featuresRoot, id, "feature.json")))
      .sort();
  } catch (error) {
    console.warn(`WARN: Could not list Linux features at ${featuresRoot}: ${error.message}`);
    return [];
  }
}

function enabledLinuxFeatureIds(options = {}) {
  const featuresRoot = linuxFeaturesRoot(options);
  const config = readLinuxFeaturesConfig(featuresRoot);
  const disabled = new Set(config.disabled);
  const featuresById = new Map();
  const seen = new Set();
  const ids = [];

  const featureForId = (id) => {
    if (!featuresById.has(id)) {
      featuresById.set(id, loadLinuxFeatureManifest(featuresRoot, id));
    }
    return featuresById.get(id);
  };

  const addFeature = (id) => {
    if (disabled.has(id) || seen.has(id)) {
      return;
    }
    seen.add(id);
    ids.push(id);
  };

  for (const id of discoverLinuxFeatureIds(featuresRoot)) {
    const feature = featureForId(id);
    if (feature?.manifest.defaultEnabled === true) {
      addFeature(id);
    }
  }
  for (const id of config.enabled) {
    if (featureForId(id) != null) {
      addFeature(id);
    }
  }
  return ids;
}

function resolvedLinuxFeaturesConfig(options = {}) {
  const featuresRoot = linuxFeaturesRoot(options);
  const knownFeatureIds = new Set(discoverLinuxFeatureIds(featuresRoot));
  const config = readLinuxFeaturesConfig(featuresRoot);
  const enabled = enabledLinuxFeatureIds({ ...options, featuresRoot });
  const disabled = config.disabled.filter((id) => knownFeatureIds.has(id));
  return { enabled, disabled };
}

function loadLinuxFeatureManifest(featuresRoot, id) {
  const featureDir = path.join(featuresRoot, id);
  const manifestPath = path.join(featureDir, "feature.json");
  if (!fs.existsSync(manifestPath)) {
    console.warn(`WARN: Linux feature '${id}' does not have feature.json`);
    return null;
  }

  const manifest = readJsonFile(manifestPath, `Linux feature '${id}' manifest`);
  if (manifest == null) {
    return null;
  }
  if (manifest.id !== id) {
    console.warn(`WARN: Linux feature '${id}' manifest id mismatch: ${String(manifest.id)}`);
    return null;
  }

  return { id, dir: featureDir, manifestPath, manifest };
}

function loadEnabledLinuxFeatures(options = {}) {
  const featuresRoot = linuxFeaturesRoot(options);
  return enabledLinuxFeatureIds({ ...options, featuresRoot })
    .map((id) => loadLinuxFeatureManifest(featuresRoot, id))
    .filter(Boolean);
}

function resolveFeatureEntrypoint(feature, key) {
  const relativePath = feature.manifest.entrypoints?.[key];
  if (relativePath == null) {
    return null;
  }
  if (typeof relativePath !== "string" || relativePath.trim().length === 0) {
    console.warn(`WARN: Linux feature '${feature.id}' has invalid ${key} entrypoint`);
    return null;
  }
  if (path.isAbsolute(relativePath) || relativePath.split(/[\\/]/).includes("..")) {
    console.warn(`WARN: Linux feature '${feature.id}' ${key} entrypoint must stay inside the feature directory`);
    return null;
  }
  const entrypoint = path.resolve(feature.dir, relativePath);
  if (!fs.existsSync(entrypoint)) {
    console.warn(`WARN: Linux feature '${feature.id}' ${key} entrypoint not found: ${entrypoint}`);
    return null;
  }
  return entrypoint;
}

function loadFeatureEntrypointModule(feature, key) {
  const entrypoint = resolveFeatureEntrypoint(feature, key);
  if (entrypoint == null) {
    return null;
  }

  try {
    return {
      entrypoint,
      moduleExports: require(entrypoint),
    };
  } catch (error) {
    console.warn(`WARN: Could not load Linux feature '${feature.id}' ${key}: ${error.message}`);
    return null;
  }
}

function featureContext(context, feature) {
  return { ...context, feature };
}

function prefixedFeaturePatchId(feature, descriptorId) {
  return descriptorId.startsWith(`feature:${feature.id}`)
    ? descriptorId
    : `feature:${feature.id}:${descriptorId}`;
}

function wrapFeaturePatchDescriptor(feature, descriptor, sourcePath, index, featureIndex) {
  if (descriptor == null || typeof descriptor !== "object") {
    console.warn(`WARN: Linux feature '${feature.id}' patch descriptor ${index + 1} must be an object`);
    return null;
  }
  if (typeof descriptor.apply !== "function") {
    console.warn(`WARN: Linux feature '${feature.id}' patch descriptor ${index + 1} must export apply`);
    return null;
  }

  const descriptorId = descriptor.id ?? descriptor.name;
  if (typeof descriptorId !== "string" || descriptorId.length === 0) {
    console.warn(`WARN: Linux feature '${feature.id}' patch descriptor ${index + 1} must have id or name`);
    return null;
  }

  const wrappedId = prefixedFeaturePatchId(feature, descriptorId);
  const wrapped = {
    ...descriptor,
    id: wrappedId,
    name: descriptor.name ?? wrappedId,
    ciPolicy: descriptor.ciPolicy ?? "optional",
    order: descriptor.order ?? 20_000 + featureIndex * 100 + index * 10,
    sourcePath,
    apply: (target, context) => descriptor.apply(target, featureContext(context, feature)),
  };

  if (typeof descriptor.appliesTo === "function") {
    wrapped.appliesTo = (context) => descriptor.appliesTo(featureContext(context, feature));
  }
  if (typeof descriptor.enabled === "function") {
    wrapped.enabled = (context) => descriptor.enabled(featureContext(context, feature));
  }
  if (typeof descriptor.targetSummary === "function") {
    wrapped.targetSummary = (context) => descriptor.targetSummary(featureContext(context, feature));
  }
  if (typeof descriptor.status === "function") {
    wrapped.status = (result, warnings, context) =>
      descriptor.status(result, warnings, featureContext(context, feature));
  }

  return wrapped;
}

function featurePatchDescriptorListFromExports(feature, moduleExports, sourcePath, featureIndex) {
  const exported = moduleExports?.descriptors ??
    moduleExports?.patches ??
    moduleExports?.default ??
    moduleExports;
  if (exported == null) {
    console.warn(`WARN: Linux feature '${feature.id}' patchDescriptors entrypoint must export descriptors`);
    return [];
  }

  const descriptors = Array.isArray(exported) ? exported : [exported];
  return descriptors
    .map((descriptor, index) =>
      wrapFeaturePatchDescriptor(feature, descriptor, sourcePath, index, featureIndex),
    )
    .filter(Boolean);
}

function loadLinuxFeaturePatchDescriptors(options = {}) {
  const descriptors = [];
  for (const [featureIndex, feature] of loadEnabledLinuxFeatures(options).entries()) {
    const loaded = loadFeatureEntrypointModule(feature, "patchDescriptors") ??
      loadFeatureEntrypointModule(feature, "patches");
    if (loaded == null) {
      const legacyLoaded = loadFeatureEntrypointModule(feature, "mainBundlePatch");
      if (legacyLoaded == null) {
        continue;
      }

      const moduleExports = legacyLoaded.moduleExports;
      const apply = moduleExports.applyMainBundlePatch ?? moduleExports.apply ?? moduleExports;
      if (typeof apply !== "function") {
        console.warn(`WARN: Linux feature '${feature.id}' mainBundlePatch must export a function`);
        continue;
      }

      descriptors.push({
        id: `feature:${feature.id}`,
        name: `feature:${feature.id}`,
        phase: "main-bundle",
        ciPolicy: "optional",
        apply: (source, context) => apply(source, featureContext(context, feature)),
      });
      continue;
    }
    descriptors.push(
      ...featurePatchDescriptorListFromExports(
        feature,
        loaded.moduleExports,
        loaded.entrypoint,
        featureIndex,
      ),
    );
  }
  return descriptors;
}

function loadLinuxFeatureMainBundlePatches(options = {}) {
  return loadLinuxFeaturePatchDescriptors(options)
    .filter((patch) => (patch.phase ?? "main-bundle") === "main-bundle")
    .map(({ apply, ciPolicy, id, name }) => ({ apply, ciPolicy, id, name }));
}

function enabledLinuxFeatureStageHooks(options = {}) {
  return loadEnabledLinuxFeatures(options)
    .map((feature) => ({
      id: feature.id,
      path: resolveFeatureEntrypoint(feature, "stageHook"),
    }))
    .filter((hook) => hook.path != null);
}

function main() {
  const command = process.argv[2];
  if (command === "--stage-hooks") {
    for (const hook of enabledLinuxFeatureStageHooks()) {
      process.stdout.write(`${hook.id}\t${hook.path}\n`);
    }
    return;
  }
  if (command === "--enabled") {
    for (const id of enabledLinuxFeatureIds()) {
      process.stdout.write(`${id}\n`);
    }
    return;
  }
  console.error("Usage: linux-features.js --enabled | --stage-hooks");
  process.exit(1);
}

if (require.main === module) {
  main();
}

module.exports = {
  enabledLinuxFeatureIds,
  enabledLinuxFeatureStageHooks,
  loadEnabledLinuxFeatures,
  loadLinuxFeaturePatchDescriptors,
  loadLinuxFeatureMainBundlePatches,
  linuxFeaturesConfigPath,
  linuxFeaturesRoot,
  linuxFeaturesUserConfigPath,
  resolvedLinuxFeaturesConfig,
  resolveFeatureEntrypoint,
};
