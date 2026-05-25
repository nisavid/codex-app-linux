#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const {
  validateReadmeVisualsContent,
} = require("./validate-readme-visuals.js");

function errorsFor(markdown) {
  return validateReadmeVisualsContent(markdown).errors;
}

test("accepts the existing app icon and shields.io badges", () => {
  const markdown = `
<div align="center">
  <img src="assets/codex.png" alt="Codex app icon" width="128" height="128">
  <p>
    <a href="#quick-start"><img alt="Packages" src="https://img.shields.io/badge/packages-deb-2f81f7?style=flat-square"></a>
    <a href="#releases"><img alt="Release" src="https://img.shields.io/github/v/release/nisavid/codex-app-linux"></a>
  </p>
</div>
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("accepts local showcase images under docs/assets/readme with alt text", () => {
  const markdown = `
![Codex workbench on a Linux desktop](docs/assets/readme/workbench.png)
![Codex workbench with angle destination](<docs/assets/readme/workbench-angle.png>)

<img src="docs/assets/readme/browser-use-annotations.webp" alt="Browser Use annotations in Codex">
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("accepts local reference-style showcase images with alt text", () => {
  const markdown = `
![Codex workbench on Linux][workbench]
![Browser Use annotations][]
![Diff view with change summary]

[workbench]: docs/assets/readme/workbench.png
[Browser Use annotations]: docs/assets/readme/browser-use-annotations.webp
[Diff view with change summary]: docs/assets/readme/diff-view.png
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("rejects local showcase images outside docs/assets/readme", () => {
  const markdown = `
![Codex workbench](assets/workbench.png)
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must live under docs/assets/readme/: assets/workbench.png",
  ]);
});

test("rejects reference-style showcase image sources outside policy", () => {
  const markdown = `
![Remote showcase][remote]
![Out-of-scope showcase][outside]
![Multiline remote showcase][multiline-remote]
![Whitespace remote showcase] [whitespace-remote]
> ![Blockquote remote showcase][blockquote-remote]

[remote]: https://example.com/workbench.png
[outside]: assets/workbench.png
[multiline-remote]:
  https://example.com/multiline-workbench.png
[whitespace-remote]: https://example.com/whitespace-workbench.png
> [blockquote-remote]: https://example.com/blockquote-workbench.png
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench.png",
    "README showcase image must live under docs/assets/readme/: assets/workbench.png",
    "README showcase image must be a local repo asset, not an external URL: https://example.com/multiline-workbench.png",
    "README showcase image must be a local repo asset, not an external URL: https://example.com/whitespace-workbench.png",
    "README showcase image must be a local repo asset, not an external URL: https://example.com/blockquote-workbench.png",
  ]);
});

test("rejects showcase paths that escape docs/assets/readme", () => {
  const markdown = `
![Escaped showcase](docs/assets/readme/../outside.png)
![Encoded escaped showcase](docs/assets/readme/%2e%2e/outside.png)
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must live under docs/assets/readme/: docs/assets/readme/../outside.png",
    "README showcase image must live under docs/assets/readme/: docs/assets/readme/%2e%2e/outside.png",
  ]);
});

test("rejects showcase images without useful alt text", () => {
  const markdown = `
![](docs/assets/readme/workbench.png)
<img src="docs/assets/readme/browser-use.png">
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image is missing alt text: docs/assets/readme/workbench.png",
    "README showcase image is missing alt text: docs/assets/readme/browser-use.png",
  ]);
});

test("rejects external showcase image URLs", () => {
  const markdown = `
![Remote showcase](https://example.com/workbench.png)
![Remote showcase with title](https://example.com/workbench-title.png 'title')
![Remote showcase with angle destination](<https://example.com/workbench-angle.png>)
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench.png",
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench-title.png",
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench-angle.png",
  ]);
});

test("reports external URLs before checking alt text", () => {
  const markdown = `
![](https://example.com/workbench.png)
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench.png",
  ]);
});

test("rejects generated or runtime artifact paths even when local", () => {
  const markdown = `
![Generated app screenshot](codex-app/screenshot.png)
![Package output screenshot](dist/workbench.png)
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must live under docs/assets/readme/: codex-app/screenshot.png",
    "README showcase image must not reference generated or runtime artifacts: codex-app/screenshot.png",
    "README showcase image must live under docs/assets/readme/: dist/workbench.png",
    "README showcase image must not reference generated or runtime artifacts: dist/workbench.png",
  ]);
});

test("ignores image-like syntax inside fenced code blocks", () => {
  const markdown = `
\`\`\`markdown
![External example](https://example.com/workbench.png)
<img src="assets/out-of-scope.png">
\`\`\`
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("ignores image-like syntax inside inline code spans", () => {
  const markdown = `
Use \`![External example](https://example.com/workbench.png)\` when documenting Markdown syntax.
Use \`<img src="assets/out-of-scope.png">\` when documenting HTML syntax.
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("preserves inline code spans inside image alt text", () => {
  const markdown = `
![\`Codex\`](docs/assets/readme/workbench.png)
<img src="docs/assets/readme/browser-use.png" alt="\`Browser Use\`">
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("ignores escaped Markdown image syntax", () => {
  const markdown = `
\\![External example](https://example.com/workbench.png)
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("validates HTML source srcset image references", () => {
  const markdown = `
<picture>
  <source srcset="https://example.com/workbench.avif 1x, docs/assets/readme/workbench.webp 2x" type="image/avif">
  <img src="docs/assets/readme/workbench.png" alt="Codex workbench">
</picture>
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench.avif",
  ]);
});

test("does not strip images between backticks on different lines", () => {
  const markdown = `
Opening \` marker on one line.
![Remote showcase](https://example.com/workbench.png)
Closing \` marker on another line.
`;

  assert.deepEqual(errorsFor(markdown), [
    "README showcase image must be a local repo asset, not an external URL: https://example.com/workbench.png",
  ]);
});

test("reports unreadable README paths without a stack trace", () => {
  const scriptPath = path.join(__dirname, "validate-readme-visuals.js");
  const result = spawnSync(process.execPath, [scriptPath, "missing-readme.md"], {
    cwd: __dirname,
    encoding: "utf8",
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /validate-readme-visuals\.js: missing-readme\.md: /);
  assert.doesNotMatch(result.stderr, /\n\s+at /);
});
