#!/usr/bin/env node
"use strict";

const assert = require("node:assert/strict");
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
  </p>
</div>
`;

  assert.deepEqual(errorsFor(markdown), []);
});

test("accepts local showcase images under docs/assets/readme with alt text", () => {
  const markdown = `
![Codex workbench on a Linux desktop](docs/assets/readme/workbench.png)

<img src="docs/assets/readme/browser-use-annotations.webp" alt="Browser Use annotations in Codex">
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
