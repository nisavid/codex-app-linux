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
      const match = stripReferenceContainerMarkers(line).match(/^ {0,3}(```+|~~~+)/);
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

function isEscapedCharacter(content, index) {
  let backslashCount = 0;
  for (let cursor = index - 1; cursor >= 0 && content[cursor] === "\\"; cursor -= 1) {
    backslashCount += 1;
  }
  return backslashCount % 2 === 1;
}

function countBacktickRun(content, index) {
  let cursor = index;
  while (content[cursor] === "`") {
    cursor += 1;
  }
  return cursor - index;
}

function findClosingBacktickRun(content, index, length) {
  for (let cursor = index; cursor < content.length && content[cursor] !== "\n"; ) {
    if (content[cursor] !== "`" || isEscapedCharacter(content, cursor)) {
      cursor += 1;
      continue;
    }

    const runLength = countBacktickRun(content, cursor);
    if (runLength === length) {
      return cursor;
    }
    cursor += runLength;
  }
  return -1;
}

function findInlineCodeSpans(content) {
  const spans = [];
  for (let cursor = 0; cursor < content.length; cursor += 1) {
    if (content[cursor] !== "`" || isEscapedCharacter(content, cursor)) {
      continue;
    }

    const length = countBacktickRun(content, cursor);
    const closing = findClosingBacktickRun(content, cursor + length, length);
    if (closing === -1) {
      cursor += length - 1;
      continue;
    }

    spans.push({ start: cursor, end: closing + length });
    cursor = closing + length - 1;
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
  return isEscapedCharacter(content, index);
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

function referenceDefinitionStart(line) {
  let cursor = 0;
  while (cursor < 3 && (line[cursor] === " " || line[cursor] === "\t")) {
    cursor += 1;
  }
  return line[cursor] === "[" ? cursor : -1;
}

function parseReferenceDefinitionSource(lines, lineIndex, sourceStart) {
  let sourceText = lines[lineIndex].slice(sourceStart).replace(/^[ \t]+/, "");
  if (sourceText.length === 0) {
    const continuation = lines[lineIndex + 1];
    if (continuation == null || !/^[ \t]+/.test(continuation)) {
      return null;
    }
    sourceText = continuation.replace(/^[ \t]+/, "");
  }

  let sourceEnd;
  if (sourceText[0] === "<") {
    sourceEnd = sourceText.indexOf(">");
    if (sourceEnd === -1) {
      return null;
    }
    sourceEnd += 1;
  } else {
    sourceEnd = sourceText.search(/[ \t]/);
    if (sourceEnd === -1) {
      sourceEnd = sourceText.length;
    }
  }

  const source = sourceText.slice(0, sourceEnd);
  const trailing = sourceText.slice(sourceEnd);
  if (trailing.length === 0 || /^[ \t]+(?:"[^"]*"|'[^']*'|\([^)]*\))[ \t]*$/.test(trailing)) {
    return normalizeReferenceSrc(source);
  }
  return null;
}

function findReferenceDefinitions(content) {
  const references = new Map();
  const lines = normalizeReferenceDefinitionContainers(content).split("\n");
  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex];
    const openIndex = referenceDefinitionStart(line);
    if (openIndex === -1) {
      continue;
    }
    const labelEnd = findPlainReferenceLabelEnd(line, openIndex);
    if (labelEnd === -1 || line[labelEnd + 1] !== ":") {
      continue;
    }

    const label = normalizeReferenceLabel(line.slice(openIndex + 1, labelEnd));
    const src = parseReferenceDefinitionSource(lines, lineIndex, labelEnd + 2);
    if (src == null) {
      continue;
    }
    if (!references.has(label)) {
      references.set(label, src);
    }
  }
  return references;
}

function findClosingMarkdownLabelBracket(content, openIndex) {
  let depth = 1;
  for (let cursor = openIndex + 1; cursor < content.length; cursor += 1) {
    if (content[cursor] === "\n") {
      return -1;
    }
    if (isEscapedCharacter(content, cursor)) {
      continue;
    }
    if (content[cursor] === "[") {
      depth += 1;
      continue;
    }
    if (content[cursor] === "]") {
      depth -= 1;
      if (depth === 0) {
        return cursor;
      }
    }
  }
  return -1;
}

function isInlineImageDestinationPadding(character) {
  return character === " " || character === "\t" || character === "\n" || character === "\r";
}

function skipInlineImageDestinationPadding(content, index) {
  let cursor = index;
  while (isInlineImageDestinationPadding(content[cursor])) {
    cursor += 1;
  }
  return cursor;
}

function parseInlineImageDestination(content, labelEnd) {
  if (content[labelEnd + 1] !== "(") {
    return null;
  }

  let cursor = skipInlineImageDestinationPadding(content, labelEnd + 2);
  if (content[cursor] === "<") {
    const destinationStart = cursor;
    for (cursor += 1; cursor < content.length && content[cursor] !== "\n"; cursor += 1) {
      if (content[cursor] !== ">" || isEscapedCharacter(content, cursor)) {
        continue;
      }
      const end = findInlineImageEndAfterDestination(content, cursor + 1);
      return end == null
        ? null
        : {
            end,
            src: normalizeReferenceSrc(content.slice(destinationStart, cursor + 1)),
          };
    }
    return null;
  }

  const destinationStart = cursor;
  let depth = 0;
  for (; cursor < content.length; cursor += 1) {
    if (isEscapedCharacter(content, cursor)) {
      continue;
    }
    if (content[cursor] === "(") {
      depth += 1;
      continue;
    }
    if (content[cursor] === ")") {
      if (depth === 0) {
        return {
          end: cursor + 1,
          src: content.slice(destinationStart, cursor),
        };
      }
      depth -= 1;
      continue;
    }
    if (isInlineImageDestinationPadding(content[cursor]) && depth === 0) {
      const end = findInlineImageEndAfterDestination(content, cursor);
      return end == null
        ? null
        : {
            end,
            src: content.slice(destinationStart, cursor),
          };
    }
  }
  return null;
}

function findInlineImageEndAfterDestination(content, index) {
  const cursor = skipInlineImageDestinationPadding(content, index);
  if (content[cursor] === ")") {
    return cursor + 1;
  }

  const titleMatch = content.slice(cursor).match(/^(?:"[^"]*"|'[^']*'|\([^)]*\))[ \t]*\)/);
  if (titleMatch == null) {
    return null;
  }
  return cursor + titleMatch[0].length;
}

function findPlainReferenceLabelEnd(content, openIndex) {
  let depth = 1;
  for (let cursor = openIndex + 1; cursor < content.length; cursor += 1) {
    if (content[cursor] === "\n") {
      return -1;
    }
    if (isEscapedCharacter(content, cursor)) {
      continue;
    }
    if (content[cursor] === "[") {
      depth += 1;
      continue;
    }
    if (content[cursor] === "]") {
      depth -= 1;
      if (depth === 0) {
        return cursor;
      }
    }
  }
  return -1;
}

function parseReferenceImageDestination(content, labelEnd, alt, references) {
  let cursor = labelEnd + 1;
  while (content[cursor] === " " || content[cursor] === "\t") {
    cursor += 1;
  }
  if (content[cursor] === "(") {
    return null;
  }

  let referenceLabel = alt;
  let end = labelEnd + 1;
  if (content[cursor] === "[") {
    const referenceEnd = findPlainReferenceLabelEnd(content, cursor);
    if (referenceEnd === -1) {
      return null;
    }
    const explicitReferenceLabel = content.slice(cursor + 1, referenceEnd);
    referenceLabel = explicitReferenceLabel.length === 0 ? alt : explicitReferenceLabel;
    end = referenceEnd + 1;
  }

  const src = references.get(normalizeReferenceLabel(referenceLabel));
  if (src == null) {
    return null;
  }
  return { end, src: src.trim() };
}

function findMarkdownImages(content, inlineCodeSpans = []) {
  const images = [];
  const references = findReferenceDefinitions(content);

  for (let cursor = 0; cursor < content.length - 1; cursor += 1) {
    if (content[cursor] !== "!" || content[cursor + 1] !== "[") {
      continue;
    }
    if (isInsideInlineCodeSpan(inlineCodeSpans, cursor)) {
      continue;
    }
    if (isEscapedMarkdownImage(content, cursor)) {
      continue;
    }

    const labelEnd = findClosingMarkdownLabelBracket(content, cursor + 1);
    if (labelEnd === -1) {
      continue;
    }
    const alt = content.slice(cursor + 2, labelEnd).trim();

    const inlineDestination = parseInlineImageDestination(content, labelEnd);
    if (inlineDestination != null) {
      images.push({
        alt,
        src: inlineDestination.src,
      });
      cursor = inlineDestination.end - 1;
      continue;
    }

    const referenceDestination = parseReferenceImageDestination(content, labelEnd, alt, references);
    if (referenceDestination != null) {
      images.push({
        alt,
        src: referenceDestination.src,
      });
      cursor = referenceDestination.end - 1;
      continue;
    }

    cursor = labelEnd;
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
