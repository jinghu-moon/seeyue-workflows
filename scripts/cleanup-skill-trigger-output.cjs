#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const projectRoot = process.cwd();
const outputDir = path.resolve(projectRoot, "tests", "skill-triggering", "output");

if (!fs.existsSync(outputDir)) {
  fs.mkdirSync(outputDir, { recursive: true });
}

const entries = fs.readdirSync(outputDir, { withFileTypes: true });
let removed = 0;

for (const entry of entries) {
  if (entry.name === ".gitkeep") continue;
  const fullPath = path.join(outputDir, entry.name);
  fs.rmSync(fullPath, { recursive: true, force: true });
  removed += 1;
}

const keepPath = path.join(outputDir, ".gitkeep");
if (!fs.existsSync(keepPath)) {
  fs.writeFileSync(keepPath, "", "utf8");
}

console.log(`[cleanup-skill-trigger-output] removed=${removed}`);
console.log(`[cleanup-skill-trigger-output] keep=${path.relative(projectRoot, keepPath).replace(/\\/g, "/")}`);
