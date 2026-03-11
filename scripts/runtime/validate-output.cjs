#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { loadYamlFile } = require("./yaml-loader.cjs");

const OUTPUT_TEMPLATES_PATH = "workflow/output-templates.spec.yaml";

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function toArray(value) {
  return Array.isArray(value) ? value : [];
}

function pushIssue(issues, code, message, details = {}) {
  issues.push({ code, message, ...details });
}

function loadOutputTemplates(rootDir) {
  const specPath = path.join(rootDir, OUTPUT_TEMPLATES_PATH);
  if (!fs.existsSync(specPath)) {
    throw new Error(`OUTPUT_TEMPLATES_MISSING ${specPath}`);
  }
  const spec = loadYamlFile(specPath);
  if (!spec || !isObject(spec.templates)) {
    throw new Error("OUTPUT_TEMPLATES_INVALID templates missing or invalid");
  }
  return spec.templates;
}

function validateOutputEntry(entry, templates, index = null) {
  const issues = [];
  const label = index === null ? "entry" : `entry[${index}]`;

  if (!isObject(entry)) {
    pushIssue(issues, "OUTPUT_ENTRY_INVALID", `${label} must be an object.`);
    return issues;
  }

  const templateId = entry.template_id;
  if (typeof templateId !== "string" || templateId.length === 0) {
    pushIssue(issues, "OUTPUT_TEMPLATE_ID_MISSING", `${label} template_id must be a non-empty string.`);
    return issues;
  }

  const template = templates[templateId];
  if (!template) {
    pushIssue(issues, "OUTPUT_TEMPLATE_UNKNOWN", `${label} template_id ${templateId} not found.`);
    return issues;
  }

  if (typeof entry.output_level !== "string" || entry.output_level.length === 0) {
    pushIssue(issues, "OUTPUT_LEVEL_MISSING", `${label} output_level must be a non-empty string.`);
  } else if (template.output_level && entry.output_level !== template.output_level) {
    pushIssue(
      issues,
      "OUTPUT_LEVEL_MISMATCH",
      `${label} output_level ${entry.output_level} does not match template ${template.output_level}.`,
      { template_id: templateId },
    );
  }

  if (!isObject(entry.variables)) {
    pushIssue(issues, "OUTPUT_VARIABLES_INVALID", `${label} variables must be an object.`);
    return issues;
  }

  const required = toArray(template.required_variables);
  for (const key of required) {
    if (!Object.prototype.hasOwnProperty.call(entry.variables, key) || entry.variables[key] === null) {
      pushIssue(
        issues,
        "OUTPUT_MISSING_VARIABLE",
        `${label} variables missing required key ${key}.`,
        { template_id: templateId, variable: key },
      );
    }
  }

  return issues;
}

function validateOutputEntries(entries, templates) {
  const issues = [];
  const list = toArray(entries);
  for (let index = 0; index < list.length; index += 1) {
    issues.push(...validateOutputEntry(list[index], templates, index));
  }
  return { ok: issues.length === 0, issues };
}

function parseOutputLog(text) {
  const issues = [];
  const entries = [];
  const lines = text.split(/\r?\n/).filter(Boolean);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    try {
      entries.push(JSON.parse(line));
    } catch (error) {
      pushIssue(
        issues,
        "OUTPUT_LOG_INVALID_JSON",
        `output log line ${index + 1} is not valid JSON.`,
        { line: index + 1 },
      );
    }
  }
  return { entries, issues };
}

function validateOutputLogFile(rootDir, logPath) {
  const resolvedRoot = path.resolve(rootDir || path.join(__dirname, "..", ".."));
  const templates = loadOutputTemplates(resolvedRoot);
  const resolvedLog = logPath
    ? path.resolve(logPath)
    : path.join(resolvedRoot, ".ai", "workflow", "output.log");

  if (!fs.existsSync(resolvedLog)) {
    return {
      ok: false,
      issues: [{ code: "OUTPUT_LOG_MISSING", message: `output log not found: ${resolvedLog}` }],
      entries: [],
    };
  }

  const text = fs.readFileSync(resolvedLog, "utf8");
  const parsed = parseOutputLog(text);
  const validation = validateOutputEntries(parsed.entries, templates);
  const issues = [...parsed.issues, ...validation.issues];
  return { ok: issues.length === 0, issues, entries: parsed.entries };
}

function parseArgs(argv) {
  const result = {
    rootDir: path.resolve(__dirname, "..", ".."),
    logPath: null,
    entry: null,
    filePath: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--root") {
      index += 1;
      result.rootDir = path.resolve(argv[index]);
      continue;
    }
    if (token === "--log") {
      index += 1;
      result.logPath = argv[index];
      continue;
    }
    if (token === "--entry") {
      index += 1;
      result.entry = argv[index];
      continue;
    }
    if (token === "--file") {
      index += 1;
      result.filePath = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${token}`);
  }
  return result;
}

function main() {
  let args;
  try {
    args = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  try {
    if (args.logPath) {
      const result = validateOutputLogFile(args.rootDir, args.logPath);
      if (!result.ok) {
        for (const issue of result.issues) {
          console.error(`ERROR ${issue.code} ${issue.message}`);
        }
        console.error("OUTPUT_VALIDATION_FAIL");
        process.exit(1);
      }
      console.log("OUTPUT_VALIDATION_PASS");
      return;
    }

    const templates = loadOutputTemplates(args.rootDir);
    let entries = [];
    if (args.entry) {
      entries = [JSON.parse(args.entry)];
    } else if (args.filePath) {
      const raw = fs.readFileSync(path.resolve(args.filePath), "utf8");
      if (raw.trim().startsWith("[")) {
        entries = JSON.parse(raw);
      } else {
        entries = raw
          .split(/\r?\n/)
          .filter(Boolean)
          .map((line) => JSON.parse(line));
      }
    } else {
      throw new Error("No --log, --entry, or --file provided");
    }

    const result = validateOutputEntries(entries, templates);
    if (!result.ok) {
      for (const issue of result.issues) {
        console.error(`ERROR ${issue.code} ${issue.message}`);
      }
      console.error("OUTPUT_VALIDATION_FAIL");
      process.exit(1);
    }
    console.log("OUTPUT_VALIDATION_PASS");
  } catch (error) {
    console.error(`OUTPUT_VALIDATION_CRASH ${error.message}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  loadOutputTemplates,
  parseOutputLog,
  validateOutputEntry,
  validateOutputEntries,
  validateOutputLogFile,
};
