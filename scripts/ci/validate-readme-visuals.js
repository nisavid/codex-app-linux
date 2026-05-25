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

function decodePathSegment(segment) {
  try {
    return decodeURIComponent(segment);
  } catch {
    return segment;
  }
}

function decodedPathSegments(src) {
  return normalizeSrc(src)
    .split("/")
    .flatMap((segment) => decodePathSegment(segment).split("/"));
}

function canonicalizeLocalPath(src) {
  const decoded = decodedPathSegments(src).join("/");
  if (decoded.length === 0) {
    return decoded;
  }
  return path.posix.normalize(decoded).replace(/^\/+/, "");
}

function hasParentPathSegment(src) {
  return decodedPathSegments(src).includes("..");
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

function findInlineCodeSpans(content) {
  const spans = [];
  const inlineCodePattern = /(`+)[^\n]*?\1/g;
  for (const match of content.matchAll(inlineCodePattern)) {
    spans.push({ start: match.index, end: match.index + match[0].length });
  }
  return spans;
}

function isInsideInlineCodeSpan(spans, index) {
  return spans.some((span) => index >= span.start && index < span.end);
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

function isEscapedMarkdownImage(content, index) {
  let backslashCount = 0;
  for (let cursor = index - 1; cursor >= 0 && content[cursor] === "\\"; cursor -= 1) {
    backslashCount += 1;
  }
  return backslashCount % 2 === 1;
}

function stripReferenceContainerMarkers(line) {
  let stripped = line;
  let changed = true;
  while (changed) {
    changed = false;
    const withoutBlockquote = stripped.replace(/^[ \t]{0,3}>[ \t]?/, "");
    if (withoutBlockquote !== stripped) {
      stripped = withoutBlockquote;
      changed = true;
      continue;
    }

    const withoutListMarker = stripped.replace(/^[ \t]{0,3}(?:[-+*]|\d{1,9}[.)])[ \t]+/, "");
    if (withoutListMarker !== stripped) {
      stripped = withoutListMarker;
      changed = true;
    }
  }
  return stripped;
}

function normalizeReferenceDefinitionContainers(content) {
  return content
    .split(/\r?\n/)
    .map((line) => stripReferenceContainerMarkers(line))
    .join("\n");
}

function findReferenceDefinitions(content) {
  const references = new Map();
  const normalizedContent = normalizeReferenceDefinitionContainers(content);
  const referenceDefinitionPattern =
    /^[ \t]{0,3}\[([^\]\n]+)\]:[ \t]*(?:\r?\n[ \t]+)?(<[^>\n]*>|[^ \t\n]+)(?:[ \t]+(?:"[^"]*"|'[^']*'|\([^)]*\)))?[ \t]*$/gm;

  for (const match of normalizedContent.matchAll(referenceDefinitionPattern)) {
    references.set(normalizeReferenceLabel(match[1]), normalizeReferenceSrc(match[2]));
  }
  return references;
}

function findMarkdownImages(content, inlineCodeSpans = []) {
  const images = [];
  const markdownImagePattern = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\)/g;
  for (const match of content.matchAll(markdownImagePattern)) {
    if (isInsideInlineCodeSpan(inlineCodeSpans, match.index)) {
      continue;
    }
    if (isEscapedMarkdownImage(content, match.index)) {
      continue;
    }
    images.push({
      alt: match[1].trim(),
      src: normalizeReferenceSrc(match[2]),
    });
  }
  const references = findReferenceDefinitions(content);
  const referenceImagePattern = /!\[([^\]\n]*)\](?:[ \t]*\[([^\]\n]*)\])?/g;
  for (const match of content.matchAll(referenceImagePattern)) {
    if (isInsideInlineCodeSpan(inlineCodeSpans, match.index)) {
      continue;
    }
    if (content[match.index + match[0].length] === "(") {
      continue;
    }
    if (isEscapedMarkdownImage(content, match.index)) {
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

function getHtmlAttribute(tag, attributeName) {
  const pattern = new RegExp(`\\b${attributeName}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`, "i");
  const match = tag.match(pattern);
  if (match == null) {
    return null;
  }
  return (match[1] ?? match[2] ?? match[3] ?? "").trim();
}

function parseSrcset(srcset) {
  return srcset
    .split(",")
    .map((candidate) => candidate.trim().split(/\s+/)[0])
    .filter((candidate) => candidate.length > 0);
}

function findHtmlImages(content, inlineCodeSpans = []) {
  const images = [];
  const htmlImagePattern = /<(img|source)\b[^>]*>/gi;
  for (const match of content.matchAll(htmlImagePattern)) {
    if (isInsideInlineCodeSpan(inlineCodeSpans, match.index)) {
      continue;
    }
    const tag = match[0];
    const tagName = match[1].toLowerCase();
    const alt = tagName === "img" ? getHtmlAttribute(tag, "alt") ?? "" : "";
    const src = getHtmlAttribute(tag, "src");
    if (src != null) {
      images.push({
        alt,
        src,
        requiresAlt: tagName === "img",
      });
    }

    const srcset = getHtmlAttribute(tag, "srcset");
    if (srcset != null) {
      for (const candidate of parseSrcset(srcset)) {
        images.push({
          alt,
          src: candidate,
          requiresAlt: tagName === "img" && src == null,
        });
      }
    }
  }
  return images;
}

function findImages(content) {
  const renderableContent = stripFencedCodeBlocks(content);
  const inlineCodeSpans = findInlineCodeSpans(renderableContent);
  return [
    ...findMarkdownImages(renderableContent, inlineCodeSpans),
    ...findHtmlImages(renderableContent, inlineCodeSpans),
  ];
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

    if (image.requiresAlt !== false && image.alt.length === 0) {
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
