#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

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

function canonicalizeLocalPath(src) {
  const normalized = normalizeSrc(src);
  if (normalized.length === 0) {
    return normalized;
  }
  return path.posix.normalize(normalized).replace(/^\/+/, "");
}

function hasParentPathSegment(src) {
  return normalizeSrc(src).split("/").includes("..");
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
  const normalized = canonicalizeLocalPath(src);
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

function stripInlineCodeSpans(content) {
  return content.replace(/(`+)[\s\S]*?\1/g, "");
}

function normalizeReferenceLabel(label) {
  return label.trim().replace(/\s+/g, " ").toLowerCase();
}

function normalizeReferenceSrc(src) {
  const trimmed = src.trim();
  if (trimmed.startsWith("<") && trimmed.endsWith(">")) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function findReferenceDefinitions(content) {
  const references = new Map();
  const referenceDefinitionPattern =
    /^[ \t]{0,3}\[([^\]\n]+)\]:[ \t]*(<[^>\n]*>|[^ \t\n]+)(?:[ \t]+(?:"[^"]*"|'[^']*'|\([^)]*\)))?[ \t]*$/gm;

  for (const match of content.matchAll(referenceDefinitionPattern)) {
    references.set(normalizeReferenceLabel(match[1]), normalizeReferenceSrc(match[2]));
  }
  return references;
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
  const references = findReferenceDefinitions(content);
  const referenceImagePattern = /!\[([^\]\n]*)\](?:\[([^\]\n]*)\])?/g;
  for (const match of content.matchAll(referenceImagePattern)) {
    if (content[match.index + match[0].length] === "(") {
      continue;
    }

    const referenceLabel = match[2] == null || match[2].length === 0 ? match[1] : match[2];
    const src = references.get(normalizeReferenceLabel(referenceLabel));
    if (src == null) {
      continue;
    }

    images.push({
      alt: match[1].trim(),
      src: src.trim(),
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
  const renderableContent = stripInlineCodeSpans(stripFencedCodeBlocks(content));
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

    const normalized = canonicalizeLocalPath(src);
    if (hasParentPathSegment(src) || !normalized.startsWith(APPROVED_SHOWCASE_PREFIX)) {
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
