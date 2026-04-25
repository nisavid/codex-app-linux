#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const extractedDir = process.argv[2];

if (!extractedDir) {
  console.error("Usage: patch-linux-window-ui.js <extracted-app-asar-dir>");
  process.exit(1);
}

const assetsDir = path.join(extractedDir, "webview", "assets");
const iconAsset = fs
  .readdirSync(assetsDir)
  .find((name) => /^app-.*\.png$/.test(name));

if (!iconAsset) {
  console.warn(`WARN: Could not find app icon asset in ${assetsDir} — skipping all UI patches`);
  process.exit(0);
}

const buildDir = path.join(extractedDir, ".vite", "build");
const mainBundle = fs
  .readdirSync(buildDir)
  .find((name) => /^main(?:-[^.]+)?\.js$/.test(name));

if (!mainBundle) {
  console.warn(`WARN: Could not find main bundle in ${buildDir} — skipping all UI patches`);
  process.exit(0);
}

const target = path.join(buildDir, mainBundle);
let source = fs.readFileSync(target, "utf8");
const packageJsonPath = path.join(extractedDir, "package.json");
const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
const webviewAssetsDir = path.join(extractedDir, "webview", "assets");

function patchAssetFiles(filenamePattern, patchFn, missingWarnMessage) {
  if (!fs.existsSync(webviewAssetsDir)) {
    console.warn(`WARN: Could not find webview assets directory in ${webviewAssetsDir} — skipping asset patch`);
    return;
  }

  const candidates = fs
    .readdirSync(webviewAssetsDir)
    .filter((name) => filenamePattern.test(name))
    .sort();

  if (candidates.length === 0) {
    console.warn(missingWarnMessage);
    return;
  }

  for (const candidate of candidates) {
    const filePath = path.join(webviewAssetsDir, candidate);
    const currentSource = fs.readFileSync(filePath, "utf8");
    const patchedSource = patchFn(currentSource);
    if (patchedSource !== currentSource) {
      fs.writeFileSync(filePath, patchedSource, "utf8");
    }
  }
}

function applyLinuxOpaqueWindowsDefaultPatch(currentSource) {
  const mergeNeedle = "opaqueWindows:e?.opaqueWindows??n.opaqueWindows,semanticColors:";
  const mergePatch =
    "opaqueWindows:e?.opaqueWindows??(typeof navigator<`u`&&((navigator.userAgentData?.platform??navigator.platform??navigator.userAgent).toLowerCase().includes(`linux`))?!0:n.opaqueWindows),semanticColors:";

  if (currentSource.includes("opaqueWindows:e?.opaqueWindows??(typeof navigator<`u`&&")) {
    return currentSource;
  }

  if (currentSource.includes(mergeNeedle)) {
    return currentSource.replace(mergeNeedle, mergePatch);
  }

  if (currentSource.includes("opaqueWindows") && currentSource.includes("semanticColors")) {
    console.warn("WARN: Could not find Linux opaque window default insertion point — skipping settings default patch");
  }

  return currentSource;
}

function applyLinuxFileManagerPatch(currentSource) {
  const fileManagerNeedle =
    "var sa=Mi({id:`fileManager`,label:`Finder`,icon:`apps/finder.png`,kind:`fileManager`,darwin:{detect:()=>`open`,args:e=>ai(e)},win32:{label:`File Explorer`,icon:`apps/file-explorer.png`,detect:ca,args:e=>ai(e),open:async({path:e})=>la(e)}});";
  const fileManagerLinuxPatch =
    "var sa=Mi({id:`fileManager`,label:`Finder`,icon:`apps/finder.png`,kind:`fileManager`,darwin:{detect:()=>`open`,args:e=>ai(e)},win32:{label:`File Explorer`,icon:`apps/file-explorer.png`,detect:ca,args:e=>ai(e),open:async({path:e})=>la(e)},linux:{label:`File Manager`,icon:`apps/file-explorer.png`,detect:()=>`linux-file-manager`,args:e=>[e],open:async({path:e})=>{let r=ua(e)??e,i=(0,a.existsSync)(r)&&(0,a.statSync)(r).isFile()?(0,t.dirname)(r):r,o=await n.shell.openPath(i);if(o)throw Error(o)}}});";
  const fileManagerId = "id:`fileManager`";
  const fileManagerBlockEnd = "function ca(){";
  const systemDefaultLinuxNeedle = "id:`systemDefault`";

  const fileManagerStart = currentSource.indexOf(fileManagerId);
  if (fileManagerStart === -1) {
    console.error("Failed to apply Linux File Manager Patch");
    return currentSource;
  }

  const fileManagerEnd = currentSource.indexOf(fileManagerBlockEnd, fileManagerStart);
  if (fileManagerEnd === -1) {
    console.error("Failed to apply Linux File Manager Patch");
    return currentSource;
  }

  const fileManagerBlock = currentSource.slice(fileManagerStart, fileManagerEnd);
  if (fileManagerBlock.includes("linux:{")) {
    return currentSource;
  }

  if (!currentSource.includes(fileManagerNeedle)) {
    console.error("Failed to apply Linux File Manager Patch");
    return currentSource;
  }

  const patchedSource = currentSource.replace(fileManagerNeedle, fileManagerLinuxPatch);
  const patchedFileManagerEnd = patchedSource.indexOf(fileManagerBlockEnd, fileManagerStart);
  if (patchedFileManagerEnd === -1) {
    console.error("Failed to apply Linux File Manager Patch");
    return currentSource;
  }

  const patchedFileManagerBlock = patchedSource.slice(fileManagerStart, patchedFileManagerEnd);
  const systemDefaultStart = patchedSource.indexOf(systemDefaultLinuxNeedle);
  const systemDefaultBlock = systemDefaultStart === -1
    ? ""
    : patchedSource.slice(
        systemDefaultStart,
        patchedSource.indexOf("async function Wa", systemDefaultStart),
      );

  if (
    !patchedFileManagerBlock.includes("linux:{label:`File Manager`") ||
    !patchedFileManagerBlock.includes("detect:()=>`linux-file-manager`") ||
    !patchedFileManagerBlock.includes("n.shell.openPath(i)") ||
    !systemDefaultBlock.includes("linux:{detect:()=>`system-default`")
  ) {
    console.error("Failed to apply Linux File Manager Patch");
    return currentSource;
  }

  return patchedSource;
}


const windowOptionsNeedle =
  "...process.platform===`win32`?{autoHideMenuBar:!0}:{},";
const iconPathExpression =
  `process.resourcesPath+\`/../content/webview/assets/${iconAsset}\``;
const iconPathNeedle =
  `icon:${iconPathExpression}`;
const windowOptionsReplacement =
  `...process.platform===\`win32\`||process.platform===\`linux\`?{autoHideMenuBar:!0,...process.platform===\`linux\`?{${iconPathNeedle}}:{}}:{},`;

if (source.includes(windowOptionsNeedle)) {
  source = source.replace(windowOptionsNeedle, windowOptionsReplacement);
} else if (!source.includes(iconPathNeedle)) {
  console.warn("WARN: Could not find BrowserWindow autoHideMenuBar snippet — skipping window options patch");
}

const menuNeedle = "process.platform===`win32`&&D.removeMenu(),";
const menuPatch = "process.platform===`linux`&&D.setMenuBarVisibility(!1),";
const menuReplacement = `${menuPatch}${menuNeedle}`;

if (source.includes(menuNeedle) && !source.includes(menuPatch)) {
  source = source.replace(menuNeedle, menuReplacement);
} else if (!source.includes(menuPatch)) {
  console.warn("WARN: Could not find window menu visibility snippet — skipping menu patch");
}

const setIconNeedle =
  ")}),D.once(`ready-to-show`,()=>{";
const setIconPatch =
  `)}),process.platform===\`linux\`&&D.setIcon(${iconPathExpression}),D.once(\`ready-to-show\`,()=>{`;

if (source.includes(setIconNeedle) && !source.includes("&&D.setIcon(")) {
  source = source.replace(setIconNeedle, setIconPatch);
} else if (!source.includes("&&D.setIcon(")) {
  console.warn("WARN: Could not find window setIcon insertion point — skipping setIcon patch");
}

// Patch 4: Replace transparent BrowserWindow background with opaque colors on Linux.
// On macOS vibrancy handles transparency; on Linux there is no compositor equivalent,
// so the transparent background causes flickering when the window moves or on hover.
const colorConstRegex = /([A-Za-z_$][\w$]*)=`#00000000`,([A-Za-z_$][\w$]*)=`#000000`,([A-Za-z_$][\w$]*)=`#f9f9f9`/;
const colorMatch = source.match(colorConstRegex);

if (colorMatch) {
  const [, transparentVar, darkVar, lightVar] = colorMatch;

  // Capture the prefersDarkColors parameter name from the background function signature.
  const funcParamRegex = /prefersDarkColors:([A-Za-z_$][\w$]*)\}\)\{return\s*([A-Za-z_$][\w$]*)===`win32`/;
  const funcMatch = source.match(funcParamRegex);

  if (funcMatch) {
    const darkColorsParam = funcMatch[1];

    const bgNeedle =
      `backgroundMaterial:\`mica\`}:{backgroundColor:${transparentVar},backgroundMaterial:null}}`;
    const bgReplacement =
      `backgroundMaterial:\`mica\`}:process.platform===\`linux\`?{backgroundColor:${darkColorsParam}?${darkVar}:${lightVar},backgroundMaterial:null}:{backgroundColor:${transparentVar},backgroundMaterial:null}}`;

    if (source.includes(bgNeedle)) {
      source = source.replace(bgNeedle, bgReplacement);
    } else {
      console.warn("WARN: Could not find BrowserWindow background color needle — skipping background patch");
    }
  } else {
    console.warn("WARN: Could not find prefersDarkColors parameter — skipping background patch");
  }
} else {
  console.warn("WARN: Could not find color constants (#00000000, #000000, #f9f9f9) — skipping background patch");
}

source = applyLinuxFileManagerPatch(source);

fs.writeFileSync(target, source, "utf8");

patchAssetFiles(
  /^code-theme-.*\.js$/,
  applyLinuxOpaqueWindowsDefaultPatch,
  `WARN: Could not find code theme bundle in ${webviewAssetsDir} — skipping translucent sidebar default patch`,
);
patchAssetFiles(
  /^use-resolved-theme-variant-.*\.js$/,
  applyLinuxOpaqueWindowsDefaultPatch,
  `WARN: Could not find resolved theme bundle in ${webviewAssetsDir} — skipping translucent sidebar default patch`,
);

if (packageJson.desktopName !== "codex-app.desktop") {
  packageJson.desktopName = "codex-app.desktop";
  fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`, "utf8");
}

console.log("Patched Linux window, shell, and appearance behavior:", {
  target,
  mainBundle,
  iconAsset,
  desktopName: packageJson.desktopName,
});
