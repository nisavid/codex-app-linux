# README Visual Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Define the first durable capture contract for future README showcase visuals and add a narrow validator for README image reference hygiene.

**Architecture:** Keep the first PR process-focused. The maintainer document defines the reproducible capture standard, shot-list priority, privacy review, editing boundaries, and asset location. A standalone Node validator checks only README image references, so future asset-producing work has a small local gate without introducing the capture pipeline in this PR.

**Tech Stack:** Markdown maintainer docs, GitHub-flavored README image references, Node.js built-in test runner, CommonJS script under `scripts/ci/`.

---

## Task 1: README Visual Validator Tests

**Files:**
- Create: `scripts/ci/validate-readme-visuals.test.js`
- Create later in Task 2: `scripts/ci/validate-readme-visuals.js`

- [ ] **Step 1: Write the failing test**

Create `scripts/ci/validate-readme-visuals.test.js` with behavior-level cases for the current README exceptions, approved showcase path, missing alt text, external showcase URLs, generated/runtime artifact paths, fenced code blocks, Markdown image titles, and unreadable CLI input paths.

```js
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
    <a href="#releases"><img alt="Release" src="https://img.shields.io/github/v/release/nisavid/codex-app-linux"></a>
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
node --test scripts/ci/validate-readme-visuals.test.js
```

Expected: FAIL with `Cannot find module './validate-readme-visuals.js'`.

## Task 2: README Visual Validator

**Files:**
- Create: `scripts/ci/validate-readme-visuals.js`
- Test: `scripts/ci/validate-readme-visuals.test.js`

- [ ] **Step 1: Implement the validator**

Create `scripts/ci/validate-readme-visuals.js` as a standalone CommonJS script. It must export `validateReadmeVisualsContent()` and `validateReadmeVisualsFile()`, allow the current app icon and shields.io badges, allow zero showcase images, ignore fenced code examples, report unreadable input paths without a stack trace, and reject future showcase image references outside `docs/assets/readme/`.

```js
#!/usr/bin/env node
"use strict";

const fs = require("node:fs");

const APPROVED_SHOWCASE_PREFIX = "docs/assets/readme/";
const GENERATED_OR_RUNTIME_PREFIXES = [
  "codex-app/",
  "codex-dev-app/",
  "codex-cua-lab-app/",
  "dist/",
  "dist-next/",
];

function usage() {
  return "Usage: validate-readme-visuals.js <README.md>";
}

function normalizeSrc(src) {
  return src.trim().replace(/^\.?\//, "");
}

function isExternalUrl(src) {
  return /^https?:\/\//i.test(src);
}

function isAllowedBadge(src) {
  return /^https:\/\/img\.shields\.io\//i.test(src);
}

function isExistingAppIcon(src) {
  return normalizeSrc(src) === "assets/codex.png";
}

function isGeneratedOrRuntimePath(src) {
  const normalized = normalizeSrc(src);
  if (normalized === "Codex.dmg") {
    return true;
  }
  if (/^codex-[^/]+-app\//.test(normalized)) {
    return true;
  }
  return GENERATED_OR_RUNTIME_PREFIXES.some((prefix) => normalized.startsWith(prefix));
}

function stripFencedCodeBlocks(content) {
  const lines = content.split(/\r?\n/);
  let fence = null;

  return lines
    .map((line) => {
      const match = line.match(/^ {0,3}(```+|~~~+)/);
      if (match == null) {
        return fence == null ? line : "";
      }

      const marker = match[1];
      if (fence == null) {
        fence = { character: marker[0], length: marker.length };
      } else if (marker[0] === fence.character && marker.length >= fence.length) {
        fence = null;
      }
      return "";
    })
    .join("\n");
}

function findMarkdownImages(content) {
  const images = [];
  const markdownImagePattern = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\)/g;
  for (const match of content.matchAll(markdownImagePattern)) {
    images.push({
      alt: match[1].trim(),
      src: match[2].trim(),
    });
  }
  return images;
}

function findHtmlImages(content) {
  const images = [];
  const htmlImagePattern = /<img\b[^>]*>/gi;
  for (const match of content.matchAll(htmlImagePattern)) {
    const tag = match[0];
    const srcMatch = tag.match(/\bsrc\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/i);
    if (srcMatch == null) {
      continue;
    }
    const altMatch = tag.match(/\balt\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/i);
    images.push({
      alt: (altMatch?.[1] ?? altMatch?.[2] ?? altMatch?.[3] ?? "").trim(),
      src: (srcMatch[1] ?? srcMatch[2] ?? srcMatch[3] ?? "").trim(),
    });
  }
  return images;
}

function findImages(content) {
  const renderableContent = stripFencedCodeBlocks(content);
  return [...findMarkdownImages(renderableContent), ...findHtmlImages(renderableContent)];
}

function shouldIgnoreImage(src) {
  return isExistingAppIcon(src) || isAllowedBadge(src);
}

function validateReadmeVisualsContent(content) {
  const errors = [];

  for (const image of findImages(content)) {
    const src = image.src;
    if (src.length === 0 || shouldIgnoreImage(src)) {
      continue;
    }

    if (isExternalUrl(src)) {
      errors.push(`README showcase image must be a local repo asset, not an external URL: ${src}`);
      continue;
    }

    if (image.alt.length === 0) {
      errors.push(`README showcase image is missing alt text: ${src}`);
    }

    const normalized = normalizeSrc(src);
    if (!normalized.startsWith(APPROVED_SHOWCASE_PREFIX)) {
      errors.push(`README showcase image must live under docs/assets/readme/: ${src}`);
    }
    if (isGeneratedOrRuntimePath(src)) {
      errors.push(`README showcase image must not reference generated or runtime artifacts: ${src}`);
    }
  }

  return { errors };
}

function validateReadmeVisualsFile(readmePath) {
  return validateReadmeVisualsContent(fs.readFileSync(readmePath, "utf8"));
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.log(usage());
    return;
  }
  if (args.length !== 1) {
    console.error(usage());
    process.exit(1);
  }

  let errors;
  try {
    ({ errors } = validateReadmeVisualsFile(args[0]));
  } catch (error) {
    console.error(`validate-readme-visuals.js: ${args[0]}: ${error.message}`);
    process.exit(1);
  }
  if (errors.length > 0) {
    console.error("README visual validation failed:");
    for (const error of errors) {
      console.error(`- ${error}`);
    }
    process.exit(1);
  }
  console.log("README visual validation passed.");
}

if (require.main === module) {
  main();
}

module.exports = {
  APPROVED_SHOWCASE_PREFIX,
  findImages,
  validateReadmeVisualsContent,
  validateReadmeVisualsFile,
};
```

- [ ] **Step 2: Run tests and the current README gate**

Run:

```bash
node --test scripts/ci/validate-readme-visuals.test.js
node scripts/ci/validate-readme-visuals.js README.md
```

Expected: both commands pass.

## Task 3: Maintainer Capture Contract

**Files:**
- Create: `docs/maintainers/readme-visual-capture.md`
- Modify: `docs/README.md`

- [ ] **Step 1: Add the maintainer document**

Create `docs/maintainers/readme-visual-capture.md` with these sections:

- `Output Contract`: one compact composite image near the README top, committed assets under `docs/assets/readme/`, and `node scripts/ci/validate-readme-visuals.js README.md` as the reference check.
- `Capture Standard`: final assets come from a checked-in, reproducible pipeline; manual screenshots are exploration only.
- `Shot List`: must-capture main Codex workbench and Browser Use annotations; strong candidates include diff view, Linux-validated Remote Control settings, and settings side-panel views; updater is optional/future; Computer Use backend output is not prioritized for stills.
- `Demo Workspace Requirements`: public or disposable content only.
- `Security And Privacy Review`: no credentials, tokens, account identifiers, private paths, private conversations, or misleading service state; metadata inspection is reviewer-owned until automated in an asset PR.
- `Validation Scope`: the first validator checks references only, not pixels, metadata, or composition quality.

- [ ] **Step 2: Link the document from the docs index**

Add this bullet under `Maintain Packaging Or Runtime Behavior` in `docs/README.md`:

```markdown
- [README Visual Capture](maintainers/readme-visual-capture.md) defines the
  maintainer process for reproducible, non-sensitive README showcase assets.
```

## Task 4: Verification And Closeout

**Files:**
- Review: all files changed by this plan

- [ ] **Step 1: Run focused verification**

Run:

```bash
node --test scripts/ci/validate-readme-visuals.test.js
node scripts/ci/validate-readme-visuals.js README.md
git diff --check
```

Expected: all commands pass.

- [ ] **Step 2: Review the diff**

Run:

```bash
git diff -- docs/README.md docs/maintainers/readme-visual-capture.md scripts/ci/validate-readme-visuals.js scripts/ci/validate-readme-visuals.test.js docs/superpowers/plans/2026-05-25-readme-visual-capture.md
```

Expected: the diff defines the first capture contract, keeps README content unchanged except the docs index link, and does not add actual showcase assets or pipeline scaffolding.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/README.md docs/maintainers/readme-visual-capture.md scripts/ci/validate-readme-visuals.js scripts/ci/validate-readme-visuals.test.js docs/superpowers/plans/2026-05-25-readme-visual-capture.md
git commit -m "docs(readme): define visual capture contract"
```

Expected: commit succeeds after focused verification passes.
