#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

const target = process.argv[2] || "codex-app";
const root = path.resolve(target);

function usage() {
  console.error("Usage: node scripts/inspect-electron-security.js <generated-app-or-extracted-asar-dir>");
}

function walk(dir, files = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!["node_modules", ".git"].includes(entry.name)) {
        walk(entryPath, files);
      }
    } else if (/\.(cjs|html|js|mjs)$/i.test(entry.name)) {
      files.push(entryPath);
    }
  }
  return files;
}

function lineNumber(text, index) {
  return text.slice(0, index).split(/\r?\n/).length;
}

function addFinding(findings, severity, file, text, index, message) {
  findings.push({
    severity,
    file: path.relative(process.cwd(), file),
    line: lineNumber(text, index),
    message,
  });
}

function scanFile(file, findings) {
  const text = fs.readFileSync(file, "utf8");
  const checks = [
    {
      pattern: /\bnodeIntegration\s*:\s*true\b/g,
      severity: "high",
      message: "nodeIntegration: true disables Electron renderer sandboxing and exposes Node.js APIs.",
    },
    {
      pattern: /\bcontextIsolation\s*:\s*false\b/g,
      severity: "high",
      message: "contextIsolation: false weakens preload/renderer isolation.",
    },
    {
      pattern: /\bwebSecurity\s*:\s*false\b/g,
      severity: "high",
      message: "webSecurity: false disables Chromium same-origin protections.",
    },
    {
      pattern: /\bsandbox\s*:\s*false\b/g,
      severity: "high",
      message: "sandbox: false disables the renderer sandbox for this window.",
    },
    {
      pattern: /<webview\b[^>]*\bnodeintegration\b[^>]*>/gi,
      severity: "high",
      message: "<webview> enables Node.js integration.",
    },
    {
      pattern: /<webview\b[^>]*\ballowpopups\b[^>]*>/gi,
      severity: "medium",
      message: "<webview> allows popups; verify window creation is constrained.",
    },
    {
      pattern: /\bshell\.openExternal\s*\(/g,
      severity: "medium",
      message: "shell.openExternal usage requires strict URL scheme and origin validation.",
    },
  ];

  for (const check of checks) {
    for (const match of text.matchAll(check.pattern)) {
      addFinding(findings, check.severity, file, text, match.index, check.message);
    }
  }
}

function main() {
  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
    usage();
    console.error(`Target directory not found: ${root}`);
    process.exit(2);
  }

  const files = walk(root);
  const findings = [];
  for (const file of files) {
    scanFile(file, findings);
  }

  console.log(`# Electron Security Inspection`);
  console.log(`Target: ${root}`);
  console.log(`Files scanned: ${files.length}`);
  console.log("");

  if (findings.length === 0) {
    console.log("No high-confidence static findings found.");
    process.exit(0);
  }

  for (const finding of findings) {
    console.log(`- ${finding.severity.toUpperCase()} ${finding.file}:${finding.line} - ${finding.message}`);
  }

  process.exit(findings.some((finding) => finding.severity === "high") ? 1 : 0);
}

main();
