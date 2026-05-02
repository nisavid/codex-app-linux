#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const {
  COMPUTER_USE_UI_ENV_VAR,
  COMPUTER_USE_UI_SETTINGS_KEY,
  applyLinuxComputerUseFeaturePatch,
  applyLinuxComputerUseInstallFlowPatch,
  applyLinuxComputerUsePluginGatePatch,
  applyLinuxComputerUseRendererAvailabilityPatch,
  applyLinuxFileManagerPatch,
  applyLinuxQuitGuardPatch,
  applyLinuxHotkeyWindowPrewarmPatch,
  applyLinuxLaunchActionArgsPatch,
  applyLinuxMenuPatch,
  applyLinuxOpaqueBackgroundPatch,
  applyLinuxKeybindOverridesRuntimePatch,
  applyLinuxSetIconPatch,
  applyLinuxSingleInstancePatch,
  applyLinuxTrayCloseSettingPatch,
  applyLinuxTrayPatch,
  applyLinuxWindowOptionsPatch,
  isComputerUseUiEnabled,
  patchMainBundleSource,
  patchExtractedApp,
  patchPackageJson,
  resolveDesktopName,
} = require("./patch-linux-window-ui.js");

const mainBundlePrefix =
  "let n=require(`electron`),i=require(`node:path`),o=require(`node:fs`);";
const fileManagerBundle =
  "var lu=jl({id:`fileManager`,label:`Finder`,icon:`apps/finder.png`,kind:`fileManager`,darwin:{detect:()=>`open`,args:e=>il(e)},win32:{label:`File Explorer`,icon:`apps/file-explorer.png`,detect:uu,args:e=>il(e),open:async({path:e})=>du(e)}});function uu(){}";
const alreadyOpaqueBackgroundBundle =
  "process.platform===`linux`?{backgroundColor:e?t:n,backgroundMaterial:null}:{backgroundColor:r,backgroundMaterial:null}";
const opaqueBackgroundBundleWithDriftingGw =
  "var cM=`#00000000`,lM=`#000000`,uM=`#f9f9f9`;function OM(e){return e===`avatarOverlay`||e===`browserCommentPopup`}function jM({platform:e,appearance:t,opaqueWindowsEnabled:n,prefersDarkColors:r}){return e===`win32`&&!OM(t)?n?{backgroundColor:r?lM:uM,backgroundMaterial:`none`}:{backgroundColor:cM,backgroundMaterial:`mica`}:{backgroundColor:cM,backgroundMaterial:null}}function gw(e){return e.page==null?e.snapshot.url:mw(e.page)}";

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function applyPatchTwice(patchFn, source, ...args) {
  const patched = patchFn(source, ...args);
  assert.equal(patchFn(patched, ...args), patched);
  return patched;
}

function trayBundleFixture() {
  return [
    "async function Hw(e){return process.platform!==`win32`&&process.platform!==`darwin`?null:(zw=!0,Lw??Rw??(Rw=(async()=>{let r=await Ww(e.buildFlavor,e.repoRoot),i=new n.Tray(r.defaultIcon);return i})()))}",
    "async function Ww(e,t){if(process.platform===`darwin`){return null}let r=process.platform===`win32`?`.ico`:`.png`,a=Nw(e,process.platform),o=[...n.app.isPackaged?[(0,i.join)(process.resourcesPath,`${a}${r}`)]:[],(0,i.join)(t,`electron`,`src`,`icons`,`${a}${r}`)];for(let e of o){let t=n.nativeImage.createFromPath(e);if(!t.isEmpty())return{defaultIcon:t,chronicleRunningIcon:null}}return{defaultIcon:await n.app.getFileIcon(process.execPath,{size:process.platform===`win32`?`small`:`normal`}),chronicleRunningIcon:null}}",
    "var pb=class{trayMenuThreads={runningThreads:[],unreadThreads:[],pinnedThreads:[],recentThreads:[],usageLimits:[]};constructor(){this.tray={on(){},setContextMenu(){},popUpContextMenu(){}};this.onTrayButtonClick=()=>{};this.tray.on(`click`,()=>{this.onTrayButtonClick()}),this.tray.on(`right-click`,()=>{this.openNativeTrayMenu()})}async handleMessage(e){switch(e.type){case`tray-menu-threads-changed`:this.trayMenuThreads=e.trayMenuThreads;return}}openNativeTrayMenu(){this.updateChronicleTrayIcon();let e=n.Menu.buildFromTemplate(this.getNativeTrayMenuItems());e.once(`menu-will-show`,()=>{this.isNativeTrayMenuOpen=!0}),e.once(`menu-will-close`,()=>{this.isNativeTrayMenuOpen=!1,this.handleNativeTrayMenuClosed()}),this.tray.popUpContextMenu(e)}updateChronicleTrayIcon(){}getNativeTrayMenuItems(){return[]}}",
    "v&&k.on(`close`,e=>{this.persistPrimaryWindowBounds(k,f);let t=this.getPrimaryWindows(f).some(e=>e!==k);if(process.platform===`win32`&&f===`local`&&!this.isAppQuitting&&this.options.canHideLastLocalWindowToTray?.()===!0&&!t){e.preventDefault(),k.hide();return}if(process.platform===`darwin`&&!this.isAppQuitting&&!t){e.preventDefault(),k.hide()}});",
    "let E=process.platform===`win32`;E&&oe();",
  ].join("");
}

function singleInstanceBundleFixture() {
  return [
    "agentRunId:process.env.CODEX_ELECTRON_AGENT_RUN_ID?.trim()||null}});let A=Date.now();await n.app.whenReady();",
    "l(e=>{R.deepLinks.queueProcessArgs(e)||ie()});let ae=",
  ].join("");
}

function currentLaunchActionBundleFixture() {
  return [
    "let ae=e=>{e.isMinimized()&&e.restore(),e.show(),e.focus()},oe=async()=>{try{M.hotkeyWindowLifecycleManager.hide();let e=M.getPrimaryWindow(`local`)??await M.createFreshLocalWindow(`/`);if(e==null)return;ae(e)}catch(e){g.reportNonFatal(e instanceof Error?e:`Failed to open window on second instance`,{kind:`second-instance-open-window-failed`})}};",
    "l(e=>{z.deepLinks.queueProcessArgs(e)||oe()});",
    "let se=async(e,t)=>{M.hotkeyWindowLifecycleManager.hide();let n=M.getPrimaryWindow(B),r=n??await M.createFreshLocalWindow(e);r!=null&&(R.desktopNotificationManager.dismissByNavigationPath(e),n!=null&&t.navigateExistingWindow&&z.navigateToRoute(r,e),ae(r))};",
    "w(`local window ensured`,A,{hostId:B,localWindowVisible:be?.isVisible()??!1}),A=Date.now(),await z.deepLinks.flushPendingDeepLinks()",
  ].join("");
}

function computerUseGateBundleFixture() {
  return [
    "var Qt=`openai-bundled`,$t=`browser-use`,en=`chrome-internal`,tn=`computer-use`,nn=`latex-tectonic`;",
    "var $n=[{forceReload:!0,installWhenMissing:!0,name:$t,isEnabled:({features:e})=>e.browserAgentAvailable,migrate:cn},{name:en,isEnabled:({buildFlavor:e})=>rn(e)},{name:tn,isEnabled:({features:e,platform:t})=>t===`darwin`&&e.computerUse,migrate:wn},{name:nn,isEnabled:()=>!0}];",
  ].join("");
}

function computerUseFeatureBundleFixture() {
  return "function me(e,{env:t=process.env,platform:n=process.platform}={}){return n!==`win32`||t.CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE!==`1`?e:{...e,computerUse:!0,computerUseNodeRepl:!0}}";
}

function computerUseRendererAvailabilityBundleFixture() {
  return [
    "function hae(e){return e===`macOS`||e===`windows`}",
    "function LS(e){let t=(0,q.c)(10),{hostId:n,featureName:r,defaultEnabled:i}=e,a=i===void 0?!0:i,{data:o,isLoading:s}=N(Wa,n),c;t[0]===o?c=t[1]:(c=o===void 0?[]:o,t[0]=o,t[1]=c);let l=c,u;if(t[2]!==r||t[3]!==l){let e;t[5]===r?e=t[6]:(e=e=>e.name===r,t[5]=r,t[6]=e),u=l.find(e),t[2]=r,t[3]=l,t[4]=u}else u=t[4];let d=u?.enabled??a,f;return t[7]!==s||t[8]!==d?(f={enabled:d,isLoading:s},t[7]=s,t[8]=d,t[9]=f):f=t[9],f}",
    "function RS(e){let t=(0,q.c)(8),{enabled:n,hostId:r,isHostLocal:i}=e,a=n===void 0?!0:n,o=r===void 0?R:r,s=Kn(),{isLoading:c,platform:l}=Hr(),u=Vn(`1506311413`),d;t[0]===o?d=t[1]:(d={featureName:`computer_use`,hostId:o},t[0]=o,t[1]=d);let f=LS(d),p;t[2]===l?p=t[3]:(p=hae(l),t[2]=l,t[3]=p);let m=a&&i&&s===`electron`&&u&&(c||p),h=m&&!c&&f.enabled&&!f.isLoading,g=m&&f.isLoading,_=m&&(c||f.isLoading),v;return t[4]!==h||t[5]!==g||t[6]!==_?(v={available:h,isFetching:g,isLoading:_},t[4]=h,t[5]=g,t[6]=_,t[7]=v):v=t[7],v}",
  ].join("");
}

function computerUseInstallFlowBundleFixture() {
  return "function Qe({forceReloadPlugins:e,hostId:t}){let ne=f({featureName:`computer_use`,hostId:t}),re=!ne.isLoading&&ne.enabled,[L,R]=(0,Z.useState)({});return re}";
}

function currentLaunchActionBundleFixture() {
  return [
    "const e={gr:e=>({default:e,...e})};let n=require(`electron`);let i=require(`node:path`);i=e.gr(i);let o=require(`node:fs`);o=e.gr(o);let f=require(`node:net`);f=e.gr(f);",
    "async function CN(){let{setSecondInstanceArgsHandler:l}=t.y(),g={reportNonFatal(){}},k=new t.In;k.add(x);let j={globalState:{get(){return true}},repoRoot:`/tmp`,codexHome:`/tmp`},M={hotkeyWindowLifecycleManager:{hide(){},ensureHotkeyWindowController(){}},getPrimaryWindow(){},createFreshLocalWindow(){},ensureHostWindow(){},windowManager:{sendMessageToWindow(){}}},B=`local`,R={desktopNotificationManager:{dismissByNavigationPath(){}},getOrCreateContext(){},localHost:B},z={deepLinks:{queueProcessArgs(){},flushPendingDeepLinks(){}},navigateToRoute(){}};let A=Date.now(),w=()=>{},ae=e=>{e.isMinimized()&&e.restore(),e.show(),e.focus()},oe=async()=>{try{M.hotkeyWindowLifecycleManager.hide();let e=M.getPrimaryWindow(`local`)??await M.createFreshLocalWindow(`/`);if(e==null)return;ae(e)}catch(e){g.reportNonFatal(e instanceof Error?e:`Failed to open window on second instance`,{kind:`second-instance-open-window-failed`})}};l(e=>{z.deepLinks.queueProcessArgs(e)||oe()});let se=async(e,t)=>{M.hotkeyWindowLifecycleManager.hide();let n=M.getPrimaryWindow(B),r=n??await M.createFreshLocalWindow(e);r!=null&&(R.desktopNotificationManager.dismissByNavigationPath(e),n!=null&&t.navigateExistingWindow&&z.navigateToRoute(r,e),ae(r))};let ce=async()=>{};E&&ce();let be=await M.ensureHostWindow(B);be&&ae(be),w(`local window ensured`,A,{hostId:B,localWindowVisible:be?.isVisible()??!1}),A=Date.now(),await z.deepLinks.flushPendingDeepLinks();}",
  ].join("");
}

test("adds Linux file manager support without relying on exact minified variable names", () => {
  const source = `${mainBundlePrefix}${fileManagerBundle}`;

  const patched = applyPatchTwice(applyLinuxFileManagerPatch, source);

  assert.match(patched, /linux:\{label:`File Manager`/);
  assert.match(patched, /detect:\(\)=>`linux-file-manager`/);
  assert.match(patched, /n\.shell\.openPath\(__codexOpenTarget\)/);
});

test("adds the Linux quit guard when electron/path/fs requires are split across statements", () => {
  const source =
    "const e={gr:e=>({default:e,...e})};let n=require(`electron`);let i=require(`node:path`);i=e.gr(i);let o=require(`node:fs`);o=e.gr(o);";

  const patched = applyPatchTwice(applyLinuxQuitGuardPatch, source);

  assert.match(patched, /let codexLinuxQuitInProgress=!1/);
  assert.match(patched, /codexLinuxMarkQuitInProgress=\(\)=>\{codexLinuxQuitInProgress=!0\}/);
  assert.match(patched, /codexLinuxIsQuitInProgress=\(\)=>codexLinuxQuitInProgress===!0/);
});

test("adds Linux menu hiding next to Windows removeMenu calls", () => {
  const source = "process.platform===`win32`&&k.removeMenu(),k.on(`closed`,()=>{})";
  const patched = applyPatchTwice(applyLinuxMenuPatch, source);

  assert.equal(
    patched,
    "process.platform===`linux`&&k.setMenuBarVisibility(!1),process.platform===`win32`&&k.removeMenu(),k.on(`closed`,()=>{})",
  );
});

test("recognizes already-applied Linux opaque background patch", () => {
  const patched = applyPatchTwice(applyLinuxOpaqueBackgroundPatch, alreadyOpaqueBackgroundBundle);
  assert.equal(patched, alreadyOpaqueBackgroundBundle);
});

test("uses the local transparent appearance predicate for Linux opaque backgrounds", () => {
  const patched = applyPatchTwice(
    applyLinuxOpaqueBackgroundPatch,
    opaqueBackgroundBundleWithDriftingGw,
  );

  assert.match(patched, /e===`linux`&&!OM\(t\)\?\{backgroundColor:r\?lM:uM/);
  assert.doesNotMatch(patched, /process\.platform===`linux`&&!gw\(t\)/);
});

test("adds Linux window icon handling when an icon asset is available", () => {
  const iconAsset = "app-test.png";
  const iconPathExpression = "process.resourcesPath+`/../content/webview/assets/app-test.png`";
  const windowOptionsSource = "...process.platform===`win32`?{autoHideMenuBar:!0}:{},";
  const readyToShowSource = "D.once(`ready-to-show`,()=>{})";

  const patchedWindowOptions = applyPatchTwice(
    applyLinuxWindowOptionsPatch,
    windowOptionsSource,
    iconAsset,
  );
  const patchedSetIcon = applyPatchTwice(applyLinuxSetIconPatch, readyToShowSource, iconAsset);
  const patchedMain = applyPatchTwice(
    patchMainBundleSource,
    [
      mainBundlePrefix,
      windowOptionsSource,
      "process.platform===`win32`&&k.removeMenu(),",
      readyToShowSource,
      alreadyOpaqueBackgroundBundle,
      fileManagerBundle,
      trayBundleFixture(),
      singleInstanceBundleFixture(),
    ].join(""),
    iconAsset,
  );

  assert.match(patchedWindowOptions, /process\.platform===`win32`\|\|process\.platform===`linux`/);
  assert.match(patchedWindowOptions, new RegExp(`icon:${escapeRegExp(iconPathExpression)}`));
  assert.equal(
    patchedSetIcon,
    `process.platform===\`linux\`&&D.setIcon(${iconPathExpression}),${readyToShowSource}`,
  );
  assert.match(patchedMain, new RegExp(`icon:${escapeRegExp(iconPathExpression)}`));
  assert.match(patchedMain, new RegExp(`D\\.setIcon\\(${escapeRegExp(iconPathExpression)}\\)`));
});

test("adds Linux tray support including the platform guard", () => {
  const iconPathExpression = "process.resourcesPath+`/../content/webview/assets/app-test.png`";
  const patched = applyPatchTwice(applyLinuxTrayPatch, trayBundleFixture(), iconPathExpression);

  assert.match(
    patched,
    /process\.platform!==`win32`&&process\.platform!==`darwin`&&process\.platform!==`linux`\?null:/,
  );
  assert.match(
    patched,
    new RegExp(`nativeImage\\.createFromPath\\(${escapeRegExp(iconPathExpression)}\\)`),
  );
  assert.match(patched, /\(process\.platform===`win32`\|\|process\.platform===`linux`\)&&f===`local`/);
  assert.match(
    patched,
    /\(process\.platform===`win32`\|\|process\.platform===`linux`\)&&f===`local`&&!this\.isAppQuitting&&!codexLinuxIsQuitInProgress\(\)/,
  );
  assert.match(patched, /setLinuxTrayContextMenu\(\)\{let e=n\.Menu\.buildFromTemplate/);
  assert.match(
    patched,
    /process\.platform===`linux`&&this\.setLinuxTrayContextMenu\(\),this\.tray\.on\(`click`/,
  );
  assert.match(
    patched,
    /openNativeTrayMenu\(\)\{if\(process\.platform===`linux`&&codexLinuxIsQuitInProgress\(\)\)return;/,
  );
  assert.match(patched, /if\(process\.platform===`linux`\)return;e\.once\(`menu-will-show`/);
  assert.match(
    patched,
    /this\.trayMenuThreads=e\.trayMenuThreads,process\.platform===`linux`&&!codexLinuxIsQuitInProgress\(\)&&this\.setLinuxTrayContextMenu\?\.\(\)/,
  );
  assert.match(
    patched,
    /\(E\|\|process\.platform===`linux`&&\(typeof codexLinuxIsTrayEnabled!==`function`\|\|codexLinuxIsTrayEnabled\(\)\)\)&&oe\(\);/,
  );
});

test("adds Linux tray support for current minified window and startup identifiers", () => {
  const source = [
    "v&&j.on(`close`,e=>{this.persistPrimaryWindowBounds(j,f);let t=this.getPrimaryWindows(f).some(e=>e!==j);if(process.platform===`win32`&&f===`local`&&!this.isAppQuitting&&this.options.canHideLastLocalWindowToTray?.()===!0&&!t){e.preventDefault(),j.hide();return}});",
    "async function eN(e){let t=await Ww(e.buildFlavor,e.repoRoot),r=new n.Tray(t.defaultIcon);return r}",
    "let ce$=async()=>{O=!0;try{await eN({buildFlavor:a,repoRoot:j.repoRoot})}catch(e){O=!1}};E&&ce$();",
  ].join("");

  const patched = applyPatchTwice(applyLinuxTrayPatch, source, null);

  assert.match(patched, /\(process\.platform===`win32`\|\|process\.platform===`linux`\)&&f===`local`/);
  assert.match(patched, /e\.preventDefault\(\),j\.hide\(\);return/);
  assert.match(
    patched,
    /\(E\|\|process\.platform===`linux`&&\(typeof codexLinuxIsTrayEnabled!==`function`\|\|codexLinuxIsTrayEnabled\(\)\)\)&&ce\$\(\);/,
  );
});

test("scopes dynamic tray startup matching to the tray initializer", () => {
  const source = [
    "async function aa(e){return e.buildFlavor}",
    "let startOther=async()=>{A=!0;try{await aa({buildFlavor:a})}catch(e){A=!1}};U&&startOther();",
    "async function eN(e){let t=await Ww(e.buildFlavor,e.repoRoot),r=new n.Tray(t.defaultIcon);return r}",
    "let ce$=async()=>{O=!0;try{await eN({buildFlavor:a,repoRoot:j.repoRoot})}catch(e){O=!1}};E&&ce$();",
  ].join("");

  const patched = applyPatchTwice(applyLinuxTrayPatch, source, null);

  assert.match(patched, /U&&startOther\(\);/);
  assert.doesNotMatch(
    patched,
    /\(U\|\|process\.platform===`linux`&&\(typeof codexLinuxIsTrayEnabled!==`function`\|\|codexLinuxIsTrayEnabled\(\)\)\)&&startOther\(\);/,
  );
  assert.match(
    patched,
    /\(E\|\|process\.platform===`linux`&&\(typeof codexLinuxIsTrayEnabled!==`function`\|\|codexLinuxIsTrayEnabled\(\)\)\)&&ce\$\(\);/,
  );
});

test("scopes close-to-tray already-patched detection to the handler", () => {
  const source = [
    "let unrelated=(process.platform===`win32`||process.platform===`linux`)&&x===`local`;",
    "v&&j.on(`close`,e=>{this.persistPrimaryWindowBounds(j,f);let t=this.getPrimaryWindows(f).some(e=>e!==j);if(process.platform===`win32`&&f===`local`&&!this.isAppQuitting&&this.options.canHideLastLocalWindowToTray?.()===!0&&!t){e.preventDefault(),j.hide();return}});",
  ].join("");

  const patched = applyPatchTwice(applyLinuxTrayPatch, source, null);

  assert.match(
    patched,
    /if\(\(process\.platform===`win32`\|\|process\.platform===`linux`\)&&f===`local`&&!this\.isAppQuitting&&!codexLinuxIsQuitInProgress\(\)&&this\.options\.canHideLastLocalWindowToTray\?\.\(\)===!0&&!t\)\{e\.preventDefault\(\),j\.hide\(\);return\}/,
  );
});

test("adds Linux single-instance lock and second-instance handoff", () => {
  const patched = applyPatchTwice(applyLinuxSingleInstancePatch, singleInstanceBundleFixture());

  assert.match(patched, /process\.platform===`linux`&&!n\.app\.requestSingleInstanceLock\(\)/);
  assert.match(patched, /n\.app\.quit\(\);return/);
  assert.match(patched, /codexLinuxBeforeQuitHandler=\(\)=>\{codexLinuxMarkQuitInProgress\(\)\}/);
  assert.match(patched, /n\.app\.on\(`before-quit`,codexLinuxBeforeQuitHandler\)/);
  assert.match(patched, /n\.app\.off\(`before-quit`,codexLinuxBeforeQuitHandler\)/);
  assert.match(patched, /codexLinuxSecondInstanceHandler/);
  assert.match(patched, /n\.app\.on\(`second-instance`,codexLinuxSecondInstanceHandler\)/);
  assert.match(patched, /n\.app\.off\(`second-instance`,codexLinuxSecondInstanceHandler\)/);
});

test("recognizes bootstrap-owned single-instance handoff in current bundles", () => {
  const source = "let{setSecondInstanceArgsHandler:l}=t.y();l(e=>{z.deepLinks.queueProcessArgs(e)||oe()});";
  const patched = applyPatchTwice(applyLinuxSingleInstancePatch, source);

  assert.equal(patched, source);
});

test("adds Linux launch actions through current setSecondInstanceArgsHandler bundles", () => {
  const launchPatched = applyPatchTwice(
    applyLinuxLaunchActionArgsPatch,
    currentLaunchActionBundleFixture(),
  );
  const prewarmPatched = applyPatchTwice(applyLinuxHotkeyWindowPrewarmPatch, launchPatched);

  assert.match(prewarmPatched, /codexLinuxHandleLaunchActionArgsFallback/);
  assert.match(prewarmPatched, /CODEX_APP_LAUNCH_ACTION_SOCKET/);
  assert.doesNotMatch(prewarmPatched, /CODEX_DESKTOP_LAUNCH_ACTION_SOCKET/);
  assert.match(prewarmPatched, /e\.includes\(`--new-chat`\)\?\(await se/);
  assert.match(prewarmPatched, /e\.includes\(`--quick-chat`\)\?\(await codexLinuxOpenQuickChat/);
  assert.match(prewarmPatched, /process\.platform===`linux`&&codexLinuxPrewarmHotkeyWindow\(\),A=Date\.now\(\),await z\.deepLinks\.flushPendingDeepLinks\(\)/);
  assert.match(launchPatched, /codexLinuxGetSetting=e=>process\.platform!==`linux`\|\|j\.globalState\.get\(e\)!==!1/);
  assert.match(launchPatched, /codexLinuxStartLaunchActionSocket=\(\)=>/);
  assert.match(launchPatched, /f\.default\.createServer/);
  assert.match(launchPatched, /o\.mkdirSync\(i\.default\.dirname\(e\)/);
  assert.match(launchPatched, /R\.desktopNotificationManager\.dismissByNavigationPath\(e\)/);
  assert.match(launchPatched, /codexLinuxHasDeepLink\(e\)&&z\.deepLinks\.queueProcessArgs\(e\)/);
  assert.match(launchPatched, /e\.includes\(`--prompt-chat`\)/);
  assert.match(launchPatched, /e\.includes\(`--quick-chat`\)/);
  assert.match(launchPatched, /e\.includes\(`--new-chat`\)/);
  assert.match(launchPatched, /process\.platform===`linux`&&codexLinuxStartLaunchActionSocket\(\);l\(e=>/);
  assert.doesNotMatch(launchPatched, /l\(e=>\{z\.deepLinks\.queueProcessArgs\(e\)\|\|oe\(\)\}\)/);
  assert.match(
    prewarmPatched,
    /process\.platform===`linux`&&codexLinuxPrewarmHotkeyWindow\(\),A=Date\.now\(\),await z\.deepLinks\.flushPendingDeepLinks\(\)/,
  );
});

test("adds Linux launch actions when captured window identifiers contain dollar signs", () => {
  const source = currentLaunchActionBundleFixture().replace(
    "let se=async(e,t)=>{M.hotkeyWindowLifecycleManager.hide();let n=M.getPrimaryWindow(B),r=n??await M.createFreshLocalWindow(e);r!=null&&(R.desktopNotificationManager.dismissByNavigationPath(e),n!=null&&t.navigateExistingWindow&&z.navigateToRoute(r,e),ae(r))};",
    "let se=async(e,t)=>{M.hotkeyWindowLifecycleManager.hide();let n=M.getPrimaryWindow(B),r$=n??await M.createFreshLocalWindow(e);r$!=null&&(R.desktopNotificationManager.dismissByNavigationPath(e),n!=null&&t.navigateExistingWindow&&z.navigateToRoute(r$,e),ae(r$))};",
  );

  const patched = applyPatchTwice(applyLinuxLaunchActionArgsPatch, source);

  assert.match(patched, /codexLinuxHandleLaunchActionArgs/);
  assert.match(patched, /z\.navigateToRoute\(r\$,e\),ae\(r\$\)/);
  assert.match(patched, /codexLinuxQuitInProgress=!1/);
  assert.equal(
    patched.match(/let codexLinuxQuitInProgress=!1/g)?.length ?? 0,
    1,
    "quit state should be declared exactly once",
  );
  assert.match(patched, /codexLinuxMarkQuitInProgress=\(\)=>\{codexLinuxQuitInProgress=!0\}/);
  assert.match(patched, /codexLinuxIsQuitInProgress=\(\)=>codexLinuxQuitInProgress===!0/);
  assert.match(patched, /codexLinuxGetSetting=e=>/);
  assert.match(patched, /codexLinuxHandleLaunchActionArgs=async e=>/);
  assert.match(patched, /codexLinuxHandleLaunchActionArgs=async e=>codexLinuxIsQuitInProgress\(\)\?!0:/);
  assert.match(patched, /codexLinuxHandleLaunchActionArgsFallback=\(e,t\)=>\{if\(codexLinuxIsQuitInProgress\(\)\)return;/);
  assert.match(patched, /codexLinuxStartLaunchActionSocket=\(\)=>/);
  assert.match(patched, /codexLinuxPrewarmHotkeyWindow=\(\)=>/);
  assert.match(patched, /e\.includes\(`--new-chat`\)/);
  assert.match(patched, /e\.includes\(`--quick-chat`\)/);
  assert.match(patched, /e\.includes\(`--prompt-chat`\)/);
  assert.match(patched, /e\.includes\(`--hotkey-window`\)/);
});

test("skips the launch-action patch without throwing when upstream startup architecture changes", () => {
  const source = [
    "async function Sg(){",
    "let{startedAtMs:r,setSparkleBridgeHandlers:s,setSecondInstanceArgsHandler:c}=e.o(),",
    "F=Lp({windowServices:M,ensureHostWindow:M.ensureHostWindow});",
    "e.mn().info(`Launching app`,{safe:{platform:process.platform,agentRunId:process.env.CODEX_ELECTRON_AGENT_RUN_ID?.trim()||null}});",
    "let k=Date.now();",
    "await n.app.whenReady();",
    "let M=ng({windowManager:S}),",
    "te=zf();",
    "s({onInstallUpdatesRequested:te.allowQuitTemporarilyForUpdateInstall,isTrustedIpcEvent:A});",
    "c(e=>{F.deepLinks.queueProcessArgs(e)}),",
    "k=Date.now(),",
    "F.deepLinks.registerProtocolClient(),",
    "k=Date.now();",
    "let ie=await M.ensureHostWindow(y);",
    "ie&&(ie.isMinimized()&&ie.restore(),ie.show(),ie.focus()),",
    "k=Date.now(),",
    "await F.deepLinks.flushPendingDeepLinks(),",
    "w(`startup complete`,r)}",
  ].join("");

  assert.doesNotThrow(() => applyLinuxLaunchActionArgsPatch(source));
});

test("gates current close-to-tray setting through the captured global state", () => {
  const source = "let j=KD({moduleDir:__dirname});let M=FM({buildFlavor:a,globalState:j.globalState,canHideLastLocalWindowToTray:()=>O,disposables:k});t.Mr().info(`Launching app`);";
  const patched = applyPatchTwice(applyLinuxTrayCloseSettingPatch, source);

  assert.match(
    patched,
    /canHideLastLocalWindowToTray:\(\)=>O&&\(process\.platform!==`linux`\|\|j\.globalState\.get\(`codex-linux-system-tray-enabled`\)!==!1\),disposables:k/,
  );
  assert.doesNotMatch(patched, /M\.globalState\.get/);
});

test("does not treat unrelated Linux setting references as close-to-tray patched", () => {
  const source = [
    "let j=KD({moduleDir:__dirname});",
    "let M=FM({buildFlavor:a,globalState:j.globalState,canHideLastLocalWindowToTray:()=>O,disposables:k});",
    "let codexLinuxGetSetting=e=>process.platform!==`linux`||j.globalState.get(`codex-linux-system-tray-enabled`)!==!1;",
    "t.Mr().info(`Launching app`);",
  ].join("");

  const patched = applyPatchTwice(applyLinuxTrayCloseSettingPatch, source);

  assert.match(
    patched,
    /canHideLastLocalWindowToTray:\(\)=>O&&\(process\.platform!==`linux`\|\|j\.globalState\.get\(`codex-linux-system-tray-enabled`\)!==!1\),disposables:k/,
  );
});

test("chooses the nearest globalState alias for close-to-tray settings", () => {
  const source = [
    "let stale={globalState:{get(){return false}}};",
    "let j=KD({moduleDir:__dirname});",
    "let M=FM({buildFlavor:a,globalState:j.globalState,canHideLastLocalWindowToTray:()=>O,disposables:k});",
    "t.Mr().info(`Launching app`);",
  ].join("");

  const patched = applyPatchTwice(applyLinuxTrayCloseSettingPatch, source);

  assert.match(
    patched,
    /canHideLastLocalWindowToTray:\(\)=>O&&\(process\.platform!==`linux`\|\|j\.globalState\.get\(`codex-linux-system-tray-enabled`\)!==!1\),disposables:k/,
  );
  assert.doesNotMatch(patched, /stale\.globalState\.get\(`codex-linux-system-tray-enabled`\)/);
});

test("allows bundled Computer Use on Linux as well as macOS", () => {
  const patched = applyPatchTwice(
    applyLinuxComputerUsePluginGatePatch,
    computerUseGateBundleFixture(),
  );

  assert.match(
    patched,
    /\{installWhenMissing:!0,name:tn,isEnabled:\(\{features:e,platform:t\}\)=>\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse/,
  );
  assert.doesNotMatch(patched, /t===`darwin`&&e\.computerUse/);
});

test("adds installWhenMissing to an already Linux-enabled Computer Use gate", () => {
  const source = computerUseGateBundleFixture().replace(
    "{name:tn,isEnabled:({features:e,platform:t})=>t===`darwin`&&e.computerUse,migrate:wn}",
    "{name:tn,isEnabled:({features:e,platform:t})=>(t===`darwin`||t===`linux`)&&e.computerUse,migrate:wn}",
  );

  const patched = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, source);

  assert.match(patched, /installWhenMissing:!0,name:tn/);
  assert.equal((patched.match(/installWhenMissing:!0,name:tn/g) || []).length, 1);
});

test("keeps scanning Computer Use gates after an already patched match", () => {
  const source = [
    "var tn=`computer-use`;",
    "var $n=[{installWhenMissing:!0,name:tn,isEnabled:({features:e,platform:t})=>(t===`darwin`||t===`linux`)&&e.computerUse,migrate:on},{name:tn,isEnabled:({features:n,platform:r})=>r===`darwin`&&n.computerUse,migrate:wn}];",
  ].join("");

  const patched = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, source);

  assert.match(
    patched,
    /name:tn,isEnabled:\(\{features:n,platform:r\}\)=>\(r===`darwin`\|\|r===`linux`\)&&n\.computerUse,migrate:wn/,
  );
  assert.equal((patched.match(/installWhenMissing:!0,name:tn/g) || []).length, 2);
  assert.doesNotMatch(patched, /r===`darwin`&&n\.computerUse/);
});

test("handles reordered Computer Use gate destructuring", () => {
  const darwinOnlySource = [
    "var tn=`computer-use`;",
    "var $n=[{name:tn,isEnabled:({platform:t,features:e})=>t===`darwin`&&e.computerUse,migrate:wn}];",
  ].join("");
  const alreadyLinuxEnabledSource = [
    "var tn=`computer-use`;",
    "var $n=[{installWhenMissing:!0,name:tn,isEnabled:({platform:t,features:e})=>(t===`darwin`||t===`linux`)&&e.computerUse,migrate:wn}];",
  ].join("");

  const patched = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, darwinOnlySource);

  assert.match(
    patched,
    /\{installWhenMissing:!0,name:tn,isEnabled:\(\{features:e,platform:t\}\)=>\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse,migrate:wn\}/,
  );
  assert.equal(applyPatchTwice(applyLinuxComputerUsePluginGatePatch, alreadyLinuxEnabledSource), alreadyLinuxEnabledSource);
});

test("targets literal Computer Use gate names without patching unrelated descriptors", () => {
  const source = [
    "var other=`other-plugin`;",
    "var $n=[{name:other,isEnabled:({features:e,platform:t})=>t===`darwin`&&e.computerUse,migrate:on},{name:`computer-use`,isEnabled:({platform:t,features:e})=>t===`darwin`&&e.computerUse,migrate:wn}];",
  ].join("");

  const patched = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, source);

  assert.match(patched, /name:other,isEnabled:\(\{features:e,platform:t\}\)=>t===`darwin`&&e\.computerUse,migrate:on/);
  assert.match(
    patched,
    /name:`computer-use`,isEnabled:\(\{features:e,platform:t\}\)=>\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse,migrate:wn/,
  );
});

test("handles quoted Computer Use gate names", () => {
  const boundNameSource = [
    "var tn=\"computer-use\";",
    "var $n=[{name:tn,isEnabled:({features:e,platform:t})=>t===`darwin`&&e.computerUse,migrate:wn}];",
  ].join("");
  const literalNameSource = "var $n=[{name:'computer-use',isEnabled:({platform:t,features:e})=>t===`darwin`&&e.computerUse,migrate:wn}];";

  const patchedBoundName = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, boundNameSource);
  const patchedLiteralName = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, literalNameSource);

  assert.match(patchedBoundName, /installWhenMissing:!0,name:tn/);
  assert.match(patchedBoundName, /\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse/);
  assert.match(patchedLiteralName, /installWhenMissing:!0,name:'computer-use'/);
  assert.match(patchedLiteralName, /\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse/);
});

test("patches the current Computer Use gate without touching the Windows-internal descriptor", () => {
  const source = [
    "var Ye=`browser-use`,Xe=`chrome-internal`,Ze=`computer-use`,Qe=`latex-tectonic`;",
    "var Dr=[{forceReload:!0,installWhenMissing:!0,name:Ye,isEnabled:({features:e})=>e.browserAgentAvailable,migrate:In},{forceReload:!0,name:Xe,isEnabled:({buildFlavor:e})=>Mn(e)},{name:Ze,isEnabled:({features:e,platform:t})=>t===`darwin`&&e.computerUse,migrate:Qn},{installWhenMissing:!0,name:Ze,isEnabled:({buildFlavor:e,features:n,platform:r})=>t.C.isInternal(e)&&r===`win32`&&n.computerUse},{name:Qe,isEnabled:()=>!0}];",
  ].join("");

  const patched = applyPatchTwice(applyLinuxComputerUsePluginGatePatch, source);

  assert.match(patched, /name:Ze,isEnabled:\(\{features:e,platform:t\}\)=>\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse,migrate:Qn/);
  assert.match(patched, /t\.C\.isInternal\(e\)&&r===`win32`&&n\.computerUse/);
  assert.equal((patched.match(/installWhenMissing:!0,name:Ze/g) || []).length, 2);
});

test("skips the Computer Use gate when it is recognizable but unpatchable", () => {
  const source =
    "var tn=`computer-use`;var x=[{name:tn,isEnabled:({features:e,platform:t})=>isMac(t)&&e.computerUse,migrate:wn}];";

  assert.equal(
    applyLinuxComputerUsePluginGatePatch(source),
    source,
  );
});

test("enables Computer Use desktop features on Linux", () => {
  const patched = applyPatchTwice(
    applyLinuxComputerUseFeaturePatch,
    computerUseFeatureBundleFixture(),
  );

  assert.match(
    patched,
    /return n===`linux`\?\{\.\.\.e,computerUse:!0,computerUseNodeRepl:!0\}:n!==`win32`\|\|t\.CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE!==`1`\?e:\{\.\.\.e,computerUse:!0,computerUseNodeRepl:!0\}/,
  );
  assert.match(patched, /CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE/);
});

test("shows Computer Use plugin UI on Linux without the upstream rollout flag", () => {
  const patched = applyPatchTwice(
    applyLinuxComputerUseRendererAvailabilityPatch,
    computerUseRendererAvailabilityBundleFixture(),
  );

  assert.match(patched, /function hae\(e\)\{return e===`macOS`\|\|e===`windows`\|\|e===`linux`\}/);
  assert.match(
    patched,
    /let m=a&&\(i\|\|l===`linux`\)&&s===`electron`&&\(l===`linux`\|\|u&&\(c\|\|p\)\),h=m&&!c&&\(l===`linux`\|\|f\.enabled\)&&!f\.isLoading,g=m&&l!==`linux`&&f\.isLoading,_=m&&\(c\|\|l!==`linux`&&f\.isLoading\),v;/,
  );
});

test("allows Computer Use install flow on Linux", () => {
  const patched = applyPatchTwice(
    applyLinuxComputerUseInstallFlowPatch,
    computerUseInstallFlowBundleFixture(),
  );

  assert.match(
    patched,
    /re=!ne\.isLoading&&ne\.enabled\|\|navigator\.userAgent\.includes\(`Linux`\)/,
  );
});

function withIsolatedHome(body) {
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "codex-cu-ui-test-"));
  const previousHome = process.env.HOME;
  const previousXdg = process.env.XDG_CONFIG_HOME;
  const previousFlag = process.env[COMPUTER_USE_UI_ENV_VAR];
  process.env.HOME = tempHome;
  delete process.env.XDG_CONFIG_HOME;
  delete process.env[COMPUTER_USE_UI_ENV_VAR];
  try {
    return body(tempHome);
  } finally {
    if (previousHome == null) {
      delete process.env.HOME;
    } else {
      process.env.HOME = previousHome;
    }
    if (previousXdg == null) {
      delete process.env.XDG_CONFIG_HOME;
    } else {
      process.env.XDG_CONFIG_HOME = previousXdg;
    }
    if (previousFlag == null) {
      delete process.env[COMPUTER_USE_UI_ENV_VAR];
    } else {
      process.env[COMPUTER_USE_UI_ENV_VAR] = previousFlag;
    }
    fs.rmSync(tempHome, { recursive: true, force: true });
  }
}

function writeSettingsFile(home, content) {
  const dir = path.join(home, ".config", "codex-app");
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, "settings.json"), content, "utf8");
}

test("isComputerUseUiEnabled defaults to false without env var or settings flag", () => {
  withIsolatedHome(() => {
    assert.equal(isComputerUseUiEnabled(), false);
  });
});

test("isComputerUseUiEnabled honours the env var", () => {
  withIsolatedHome(() => {
    process.env[COMPUTER_USE_UI_ENV_VAR] = "1";
    assert.equal(isComputerUseUiEnabled(), true);
    process.env[COMPUTER_USE_UI_ENV_VAR] = "true";
    assert.equal(isComputerUseUiEnabled(), false, "only the literal string '1' should opt in");
  });
});

test("isComputerUseUiEnabled honours the persisted settings flag", () => {
  withIsolatedHome((home) => {
    writeSettingsFile(home, JSON.stringify({ [COMPUTER_USE_UI_SETTINGS_KEY]: true }));
    assert.equal(isComputerUseUiEnabled(), true);
  });
});

test("isComputerUseUiEnabled treats settings flag false/missing as opt-out", () => {
  withIsolatedHome((home) => {
    writeSettingsFile(home, JSON.stringify({ [COMPUTER_USE_UI_SETTINGS_KEY]: false }));
    assert.equal(isComputerUseUiEnabled(), false);
    writeSettingsFile(home, JSON.stringify({ unrelated: true }));
    assert.equal(isComputerUseUiEnabled(), false);
  });
});

test("isComputerUseUiEnabled fails closed when settings.json is malformed", () => {
  withIsolatedHome((home) => {
    writeSettingsFile(home, "{not valid json");
    assert.equal(isComputerUseUiEnabled(), false);
  });
});

test("patchMainBundleSource skips Computer Use feature patch by default", () => {
  withIsolatedHome(() => {
    const source = [
      mainBundlePrefix,
      computerUseFeatureBundleFixture(),
      computerUseGateBundleFixture(),
    ].join("");

    const patched = patchMainBundleSource(source, null);

    assert.doesNotMatch(
      patched,
      /return n===`linux`\?\{\.\.\.e,computerUse:!0,computerUseNodeRepl:!0\}/,
    );
    assert.match(patched, /\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse/);
  });
});

test("patchMainBundleSource applies Computer Use feature patch when env var is set", () => {
  withIsolatedHome(() => {
    process.env[COMPUTER_USE_UI_ENV_VAR] = "1";
    const source = [
      mainBundlePrefix,
      computerUseFeatureBundleFixture(),
      computerUseGateBundleFixture(),
    ].join("");

    const patched = patchMainBundleSource(source, null);

    assert.match(
      patched,
      /return n===`linux`\?\{\.\.\.e,computerUse:!0,computerUseNodeRepl:!0\}/,
    );
    assert.match(patched, /\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse/);
  });
});

test("patchMainBundleSource applies Computer Use feature patch when settings.json flag is set", () => {
  withIsolatedHome((home) => {
    writeSettingsFile(home, JSON.stringify({ [COMPUTER_USE_UI_SETTINGS_KEY]: true }));
    const source = [
      mainBundlePrefix,
      computerUseFeatureBundleFixture(),
      computerUseGateBundleFixture(),
    ].join("");

    const patched = patchMainBundleSource(source, null);

    assert.match(
      patched,
      /return n===`linux`\?\{\.\.\.e,computerUse:!0,computerUseNodeRepl:!0\}/,
    );
  });
});

test("uses CODEX_APP_ID for Electron desktopName", () => {
  assert.equal(resolveDesktopName({}), "codex-app.desktop");
  assert.equal(resolveDesktopName({ CODEX_APP_ID: "codex-cua-lab" }), "codex-cua-lab.desktop");
  assert.throws(
    () => resolveDesktopName({ CODEX_APP_ID: "bad/app" }),
    /CODEX_APP_ID must contain only/,
  );

  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "codex-app-name-test-"));
  const previousAppId = process.env.CODEX_APP_ID;
  try {
    fs.writeFileSync(path.join(tempRoot, "package.json"), JSON.stringify({ name: "codex" }));
    process.env.CODEX_APP_ID = "codex-cua-lab";

    assert.equal(patchPackageJson(tempRoot), "codex-cua-lab.desktop");
    assert.equal(patchPackageJson(tempRoot), "codex-cua-lab.desktop");
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(tempRoot, "package.json"), "utf8")).desktopName,
      "codex-cua-lab.desktop",
    );
  } finally {
    if (previousAppId == null) {
      delete process.env.CODEX_APP_ID;
    } else {
      process.env.CODEX_APP_ID = previousAppId;
    }
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

test("patchMainBundleSource keeps non-icon patches active without an icon asset", () => {
  const source = [
    mainBundlePrefix,
    "process.platform===`win32`&&k.removeMenu(),",
    alreadyOpaqueBackgroundBundle,
    fileManagerBundle,
    trayBundleFixture(),
    singleInstanceBundleFixture(),
    computerUseGateBundleFixture(),
  ].join("");

  const patched = applyPatchTwice(patchMainBundleSource, source, null);

  assert.match(patched, /codexLinuxQuitInProgress=!1/);
  assert.match(patched, /codexLinuxIsQuitInProgress=\(\)=>codexLinuxQuitInProgress===!0/);
  assert.match(patched, /n\.app\.on\(`before-quit`,codexLinuxBeforeQuitHandler\)/);
  assert.match(patched, /process\.platform===`linux`&&k\.setMenuBarVisibility\(!1\)/);
  assert.match(patched, /linux:\{label:`File Manager`/);
  assert.match(
    patched,
    /process\.platform!==`win32`&&process\.platform!==`darwin`&&process\.platform!==`linux`\?null:/,
  );
  assert.match(patched, /process\.platform===`linux`&&!n\.app\.requestSingleInstanceLock\(\)/);
  assert.match(patched, /\(t===`darwin`\|\|t===`linux`\)&&e\.computerUse/);
  assert.doesNotMatch(patched, /setIcon\(process\.resourcesPath\+`\/\.\.\/content\/webview\/assets\//);
  assert.doesNotMatch(
    patched,
    /nativeImage\.createFromPath\(process\.resourcesPath\+`\/\.\.\/content\/webview\/assets\//,
  );
});

test("adds keybind override runtime with escaped source literals", () => {
  const source = "var Ct={newThread:`Ctrl+N`};";

  const patched = applyPatchTwice(applyLinuxKeybindOverridesRuntimePatch, source);

  assert.match(patched, /let storageKey="codex-linux-keybind-overrides"/);
  assert.match(patched, /codexLinuxKeybindOverridesRuntime\(\)/);
  assert.doesNotMatch(patched, /JSON\.stringify\(linuxKeybindOverridesKey\)/);
});

test("missing icon asset skips only icon patches", () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "codex-patch-test-"));
  try {
    const buildDir = path.join(tempRoot, ".vite", "build");
    const assetsDir = path.join(tempRoot, "webview", "assets");
    fs.mkdirSync(buildDir, { recursive: true });
    fs.mkdirSync(assetsDir, { recursive: true });
    fs.writeFileSync(
      path.join(buildDir, "main.js"),
      [
        mainBundlePrefix,
        "process.platform===`win32`&&k.removeMenu(),",
        alreadyOpaqueBackgroundBundle,
        fileManagerBundle,
        trayBundleFixture(),
        singleInstanceBundleFixture(),
      ].join(""),
    );
    for (const name of [
      "code-theme-test.js",
      "general-settings-test.js",
      "index-test.js",
      "use-resolved-theme-variant-test.js",
    ]) {
      fs.writeFileSync(
        path.join(assetsDir, name),
        "opaqueWindows:e?.opaqueWindows??n.opaqueWindows,semanticColors:",
      );
    }
    fs.writeFileSync(path.join(tempRoot, "package.json"), JSON.stringify({ name: "codex" }));

    patchExtractedApp(tempRoot);

    const patchedMainPath = path.join(buildDir, "main.js");
    const patchedThemePath = path.join(assetsDir, "use-resolved-theme-variant-test.js");
    const patchedPackagePath = path.join(tempRoot, "package.json");
    const patchedMain = fs.readFileSync(patchedMainPath, "utf8");
    const patchedTheme = fs.readFileSync(patchedThemePath, "utf8");
    const patchedPackageRaw = fs.readFileSync(patchedPackagePath, "utf8");
    const patchedPackage = JSON.parse(patchedPackageRaw);

    patchExtractedApp(tempRoot);

    assert.match(patchedMain, /linux:\{label:`File Manager`/);
    assert.match(patchedTheme, /includes\(`linux`\)/);
    assert.equal(patchedPackage.desktopName, "codex-app.desktop");
    assert.equal(fs.readFileSync(patchedMainPath, "utf8"), patchedMain);
    assert.equal(fs.readFileSync(patchedThemePath, "utf8"), patchedTheme);
    assert.equal(fs.readFileSync(patchedPackagePath, "utf8"), patchedPackageRaw);
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});
