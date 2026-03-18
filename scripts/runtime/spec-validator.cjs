"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { loadYamlFile } = require("./yaml-loader.cjs");

const MANIFEST_PATH = "workflow/validate-manifest.yaml";

const DEFAULT_SPEC_PATHS = [
  "workflow/validate-manifest.yaml",
  "workflow/capabilities.yaml",
  "workflow/persona-bindings.yaml",
  "workflow/file-classes.yaml",
  "workflow/approval-matrix.yaml",
  "workflow/hooks.spec.yaml",
  "workflow/hook-contract.schema.yaml",
  "workflow/skills.spec.yaml",
  "workflow/output-templates.spec.yaml",
  "workflow/runtime.schema.yaml",
  "workflow/router.spec.yaml",
  "workflow/policy.spec.yaml",
  "workflow/execution.spec.yaml",
  "workflow/interaction.schema.yaml",
];

const EXPECTED_SCHEMA_KIND = {
  "workflow/validate-manifest.yaml": "validate_manifest",
  "workflow/capabilities.yaml": "capability_registry",
  "workflow/persona-bindings.yaml": "persona_bindings",
  "workflow/file-classes.yaml": "file_class_registry",
  "workflow/approval-matrix.yaml": "approval_matrix",
  "workflow/hooks.spec.yaml": "hook_spec",
  "workflow/hook-contract.schema.yaml": "hook_contract_schema",
  "workflow/skills.spec.yaml": "skills_spec",
  "workflow/output-templates.spec.yaml": "output_template_registry",
  "workflow/runtime.schema.yaml": "runtime_schema",
  "workflow/router.spec.yaml": "router_spec",
  "workflow/policy.spec.yaml": "policy_spec",
  "workflow/execution.spec.yaml": "execution_spec",
  "workflow/interaction.schema.yaml": "interaction_schema",
};

const SPEC_STATUSES = new Set(["draft", "frozen"]);
const RISK_CLASSES = new Set(["low", "medium", "high", "critical"]);
const APPROVAL_MODES = new Set(["manual_required", "never_auto"]);
const GRANT_SCOPES = new Set(["once", "session"]);
const ENGINE_KINDS = new Set(["claude_code", "codex", "gemini_cli"]);
const PHASE_IDS = new Set(["plan", "execute", "review"]);
const TRIGGER_MODES = new Set(["explicit", "implicit", "phase_gate"]);
const HOOK_VERDICTS = new Set(["allow", "block", "block_with_approval_request", "force_continue"]);
const HOOK_AVAILABILITY = new Set(["supported", "bridged", "unsupported"]);

function toArray(value) {
  return Array.isArray(value) ? value : [];
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function normalizeSpecPath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function pushIssue(issues, code, specPath, message, severity = "error") {
  issues.push({ code, specPath, message, severity });
}

function ensureArrayOfStrings(value) {
  return Array.isArray(value) && value.every((item) => typeof item === "string" && item.length > 0);
}

function parseGateId(gateId) {
  if (typeof gateId !== "string") {
    return null;
  }
  const match = gateId.match(/^P(\d+)-N(\d+)$/i);
  if (!match) {
    return null;
  }
  return { phase: Number(match[1]), node: Number(match[2]) };
}

function compareGate(left, right) {
  const leftGate = parseGateId(left);
  const rightGate = parseGateId(right);
  if (!leftGate || !rightGate) {
    return null;
  }
  if (leftGate.phase !== rightGate.phase) {
    return leftGate.phase - rightGate.phase;
  }
  return leftGate.node - rightGate.node;
}

function listWorkflowSpecs(rootDir) {
  const workflowDir = path.join(rootDir, "workflow");
  if (!fs.existsSync(workflowDir)) {
    return [];
  }
  return fs
    .readdirSync(workflowDir)
    .filter((fileName) => fileName.endsWith(".yaml") || fileName.endsWith(".yml"))
    .map((fileName) => `workflow/${fileName}`);
}

function loadManifest(rootDir) {
  const manifestPath = path.join(rootDir, MANIFEST_PATH);
  if (!fs.existsSync(manifestPath)) {
    return null;
  }
  return loadYamlFile(manifestPath);
}

function validateManifest(manifest, issues) {
  if (!manifest) {
    pushIssue(issues, "MISSING_MANIFEST", MANIFEST_PATH, "validate-manifest.yaml is required.");
    return null;
  }
  if (!isObject(manifest)) {
    pushIssue(issues, "INVALID_MANIFEST_ROOT", MANIFEST_PATH, "manifest MUST be a mapping.");
    return null;
  }
  if (manifest.schema_kind !== "validate_manifest") {
    pushIssue(issues, "INVALID_SCHEMA_KIND", MANIFEST_PATH, "schema_kind MUST be validate_manifest.");
  }
  if (typeof manifest.schema_version !== "number" || manifest.schema_version < 1) {
    pushIssue(issues, "INVALID_SCHEMA_VERSION", MANIFEST_PATH, "schema_version MUST be a positive integer.");
  }
  if (typeof manifest.schema_dialect !== "string" || manifest.schema_dialect.length === 0) {
    pushIssue(issues, "INVALID_SCHEMA_DIALECT", MANIFEST_PATH, "schema_dialect MUST be a non-empty string.");
  }
  if (typeof manifest.owner !== "string" || manifest.owner.length === 0) {
    pushIssue(issues, "INVALID_OWNER", MANIFEST_PATH, "owner MUST be a non-empty string.");
  }
  if (!isObject(manifest.specs)) {
    pushIssue(issues, "INVALID_MANIFEST_SPECS", MANIFEST_PATH, "specs MUST be a mapping of spec entries.");
    return null;
  }

  const entries = new Map();
  for (const [specPath, entry] of Object.entries(manifest.specs)) {
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_MANIFEST_ENTRY", MANIFEST_PATH, `${specPath} entry MUST be a mapping.`);
      continue;
    }
    if (typeof entry.schema_kind !== "string" || entry.schema_kind.length === 0) {
      pushIssue(issues, "INVALID_MANIFEST_SCHEMA_KIND", MANIFEST_PATH, `${specPath} schema_kind MUST be a string.`);
    }
    if (!SPEC_STATUSES.has(entry.status)) {
      pushIssue(issues, "INVALID_MANIFEST_STATUS", MANIFEST_PATH, `${specPath} status MUST be draft or frozen.`);
    }
    if (typeof entry.schema_version !== "number" || entry.schema_version < 1) {
      pushIssue(issues, "INVALID_MANIFEST_SCHEMA_VERSION", MANIFEST_PATH, `${specPath} schema_version MUST be a positive integer.`);
    }
    if (typeof entry.freeze_gate !== "string" || entry.freeze_gate.length === 0) {
      pushIssue(issues, "INVALID_MANIFEST_FREEZE_GATE", MANIFEST_PATH, `${specPath} freeze_gate MUST be a non-empty string.`);
    }
    if (!ensureArrayOfStrings(entry.cross_refs)) {
      pushIssue(issues, "INVALID_MANIFEST_CROSS_REFS", MANIFEST_PATH, `${specPath} cross_refs MUST be a string array.`);
    }
    entries.set(normalizeSpecPath(specPath), entry);
  }

  for (const [specPath, entry] of entries.entries()) {
    for (const ref of toArray(entry.cross_refs)) {
      if (!entries.has(ref)) {
        pushIssue(issues, "INVALID_SPEC_CROSS_REF", MANIFEST_PATH, `${specPath} cross_refs unknown spec ${ref}.`);
      }
    }
  }

  return entries;
}

function collectRequestedSpecs(requestedSpecs, validateAll, manifestEntries) {
  if (requestedSpecs.length > 0) {
    return requestedSpecs.map(normalizeSpecPath);
  }
  if (validateAll) {
    if (manifestEntries && manifestEntries.size > 0) {
      return [...manifestEntries.keys()];
    }
    return DEFAULT_SPEC_PATHS;
  }
  return DEFAULT_SPEC_PATHS;
}

function loadSpecs(rootDir, specPaths) {
  const specs = new Map();
  for (const specPath of specPaths) {
    const absolutePath = path.join(rootDir, specPath);
    if (!fs.existsSync(absolutePath)) {
      specs.set(specPath, null);
      continue;
    }
    specs.set(specPath, loadYamlFile(absolutePath));
  }
  return specs;
}

function validateEnvelope(specs, manifestEntries, issues, freezeGate) {
  for (const [specPath, spec] of specs.entries()) {
    const manifestEntry = manifestEntries ? manifestEntries.get(specPath) : null;
    const expectedKind = manifestEntry?.schema_kind || EXPECTED_SCHEMA_KIND[specPath];

    if (manifestEntries && !manifestEntries.has(specPath)) {
      pushIssue(issues, "SPEC_NOT_REGISTERED", specPath, "Spec is not registered in validate-manifest.", "warning");
    }
    if (spec === null) {
      pushIssue(issues, "MISSING_REQUIRED_SPEC", specPath, "Required spec file is missing.");
      continue;
    }
    if (!isObject(spec)) {
      pushIssue(issues, "INVALID_SPEC_ROOT", specPath, "Spec root MUST be a mapping/object.");
      continue;
    }
    if (expectedKind && spec.schema_kind !== expectedKind) {
      pushIssue(issues, "INVALID_SCHEMA_KIND", specPath, `Expected schema_kind=${expectedKind}.`);
    }
    if (typeof spec.schema_version !== "number" || spec.schema_version < 1) {
      pushIssue(issues, "INVALID_SCHEMA_VERSION", specPath, "schema_version MUST be a positive integer.");
    }
    if (typeof spec.schema_dialect !== "string" || spec.schema_dialect.length === 0) {
      pushIssue(issues, "INVALID_SCHEMA_DIALECT", specPath, "schema_dialect MUST be a non-empty string.");
    }
    if (typeof spec.owner !== "string" || spec.owner.length === 0) {
      pushIssue(issues, "INVALID_OWNER", specPath, "owner MUST be a non-empty string.");
    }
    if (!SPEC_STATUSES.has(spec.status)) {
      pushIssue(issues, "INVALID_SPEC_STATUS", specPath, "status MUST be draft or frozen.");
    }
    if (manifestEntry && spec.status && manifestEntry.status !== spec.status) {
      pushIssue(issues, "SPEC_STATUS_MISMATCH", specPath, `manifest status=${manifestEntry.status} but spec status=${spec.status}.`);
    }
    if (manifestEntry && freezeGate) {
      const cmp = compareGate(manifestEntry.freeze_gate, freezeGate);
      if (cmp !== null && cmp <= 0 && spec.status !== "frozen") {
        pushIssue(issues, "SPEC_NOT_FROZEN_AT_REQUIRED_GATE", specPath, `Spec must be frozen by ${manifestEntry.freeze_gate}.`);
      }
    }
  }
}

function validateCapabilities(capabilitiesSpec, issues) {
  const specPath = "workflow/capabilities.yaml";
  if (!capabilitiesSpec || !isObject(capabilitiesSpec.capabilities)) {
    pushIssue(issues, "INVALID_CAPABILITY_REGISTRY", specPath, "capabilities MUST be a mapping.");
    return [];
  }
  const capabilityIds = Object.keys(capabilitiesSpec.capabilities);
  if (capabilityIds.length === 0) {
    pushIssue(issues, "EMPTY_CAPABILITY_REGISTRY", specPath, "At least one capability MUST be defined.");
  }
  for (const capabilityId of capabilityIds) {
    const entry = capabilitiesSpec.capabilities[capabilityId];
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_CAPABILITY_ENTRY", specPath, `${capabilityId} MUST be a mapping.`);
      continue;
    }
    if (typeof entry.category !== "string" || entry.category.length === 0) {
      pushIssue(issues, "INVALID_CAPABILITY_CATEGORY", specPath, `${capabilityId} MUST define category.`);
    }
    if (typeof entry.default_persona !== "string" || entry.default_persona.length === 0) {
      pushIssue(issues, "INVALID_CAPABILITY_DEFAULT_PERSONA", specPath, `${capabilityId} MUST define default_persona.`);
    }
    if (!ensureArrayOfStrings(entry.allowed_personas)) {
      pushIssue(issues, "INVALID_CAPABILITY_ALLOWED_PERSONAS", specPath, `${capabilityId} allowed_personas MUST be a non-empty string array.`);
    }
    if (!ensureArrayOfStrings(entry.engine_support)) {
      pushIssue(issues, "INVALID_CAPABILITY_ENGINE_SUPPORT", specPath, `${capabilityId} engine_support MUST be a non-empty string array.`);
    } else {
      for (const engine of entry.engine_support) {
        if (!ENGINE_KINDS.has(engine)) {
          pushIssue(issues, "INVALID_ENGINE_KIND", specPath, `${capabilityId} references unknown engine ${engine}.`);
        }
      }
    }
  }
  return capabilityIds;
}

function validatePersonaBindings(personaSpec, capabilityIds, issues) {
  const specPath = "workflow/persona-bindings.yaml";
  if (!personaSpec || !isObject(personaSpec.personas)) {
    pushIssue(issues, "INVALID_PERSONA_BINDINGS", specPath, "personas MUST be a mapping.");
    return [];
  }
  const personaIds = Object.keys(personaSpec.personas);
  if (personaIds.length === 0) {
    pushIssue(issues, "EMPTY_PERSONA_BINDINGS", specPath, "At least one persona MUST be defined.");
  }
  const capabilitySet = new Set(capabilityIds);
  for (const personaId of personaIds) {
    const entry = personaSpec.personas[personaId];
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_PERSONA_ENTRY", specPath, `${personaId} MUST be a mapping.`);
      continue;
    }
    if (!ensureArrayOfStrings(entry.allowed_capabilities)) {
      pushIssue(issues, "INVALID_PERSONA_CAPABILITIES", specPath, `${personaId} allowed_capabilities MUST be a non-empty string array.`);
      continue;
    }
    for (const capabilityId of entry.allowed_capabilities) {
      if (!capabilitySet.has(capabilityId)) {
        pushIssue(issues, "INVALID_CAPABILITY_REF", specPath, `${personaId} references unknown capability ${capabilityId}.`);
      }
    }
    for (const field of ["input_contract", "output_contract"]) {
      if (!ensureArrayOfStrings(entry[field])) {
        pushIssue(issues, "INVALID_PERSONA_CONTRACT", specPath, `${personaId} ${field} MUST be a non-empty string array.`);
      }
    }
  }
  if (!ensureArrayOfStrings(personaSpec.review_chain)) {
    pushIssue(issues, "INVALID_REVIEW_CHAIN", specPath, "review_chain MUST be a non-empty string array.");
  } else {
    for (const personaId of personaSpec.review_chain) {
      if (!personaIds.includes(personaId)) {
        pushIssue(issues, "INVALID_PERSONA_REF", specPath, `review_chain references unknown persona ${personaId}.`);
      }
    }
  }
  return personaIds;
}

function validateFileClasses(fileClassSpec, issues) {
  const specPath = "workflow/file-classes.yaml";
  if (!fileClassSpec || !isObject(fileClassSpec.classes)) {
    pushIssue(issues, "INVALID_FILE_CLASS_REGISTRY", specPath, "classes MUST be a mapping.");
    return [];
  }
  const classIds = Object.keys(fileClassSpec.classes);
  if (classIds.length === 0) {
    pushIssue(issues, "EMPTY_FILE_CLASS_REGISTRY", specPath, "At least one file class MUST be defined.");
  }
  for (const classId of classIds) {
    const entry = fileClassSpec.classes[classId];
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_FILE_CLASS_ENTRY", specPath, `${classId} MUST be a mapping.`);
      continue;
    }
    if (!RISK_CLASSES.has(entry.default_risk_class)) {
      pushIssue(issues, "INVALID_RISK_CLASS", specPath, `${classId} has invalid default_risk_class ${entry.default_risk_class}.`);
    }
    if (!ensureArrayOfStrings(entry.patterns)) {
      pushIssue(issues, "INVALID_FILE_CLASS_PATTERNS", specPath, `${classId} patterns MUST be a non-empty string array.`);
    }
  }
  if (!ensureArrayOfStrings(fileClassSpec.match_precedence)) {
    pushIssue(issues, "INVALID_MATCH_PRECEDENCE", specPath, "match_precedence MUST be a non-empty string array.");
  } else {
    for (const classId of fileClassSpec.match_precedence) {
      if (!classIds.includes(classId)) {
        pushIssue(issues, "INVALID_FILE_CLASS_REF", specPath, `match_precedence references unknown file class ${classId}.`);
      }
    }
  }
  return classIds;
}

function validateApprovalRuleEntry(issues, specPath, label, entry) {
  if (!isObject(entry)) {
    pushIssue(issues, "INVALID_APPROVAL_RULE", specPath, `${label} MUST be a mapping.`);
    return;
  }
  if (!RISK_CLASSES.has(entry.risk_class)) {
    pushIssue(issues, "INVALID_RISK_CLASS", specPath, `${label} has invalid risk_class ${entry.risk_class}.`);
  }
  if (typeof entry.approval_required !== "boolean") {
    pushIssue(issues, "INVALID_APPROVAL_REQUIRED", specPath, `${label} MUST define approval_required as boolean.`);
  }
  if (typeof entry.notify_only_allowed !== "boolean") {
    pushIssue(issues, "INVALID_NOTIFY_ONLY_FLAG", specPath, `${label} MUST define notify_only_allowed as boolean.`);
  }
  if (entry.approval_required) {
    if (!APPROVAL_MODES.has(entry.approval_mode)) {
      pushIssue(issues, "INVALID_APPROVAL_MODE", specPath, `${label} has invalid approval_mode ${entry.approval_mode}.`);
    }
    if (!ensureArrayOfStrings(entry.allowed_grant_scopes)) {
      pushIssue(issues, "INVALID_GRANT_SCOPE", specPath, `${label} MUST define allowed_grant_scopes.`);
    } else {
      for (const scope of entry.allowed_grant_scopes) {
        if (!GRANT_SCOPES.has(scope)) {
          pushIssue(issues, "INVALID_GRANT_SCOPE", specPath, `${label} has invalid grant scope ${scope}.`);
        }
      }
    }
  }
}

function validateApprovalMatrix(approvalSpec, fileClassIds, issues) {
  const specPath = "workflow/approval-matrix.yaml";
  if (!approvalSpec || !isObject(approvalSpec.command_classes) || !isObject(approvalSpec.file_classes)) {
    pushIssue(issues, "INVALID_APPROVAL_MATRIX", specPath, "command_classes and file_classes MUST be mappings.");
    return { commandClassIds: [], fileClassRuleIds: [] };
  }
  if (!ensureArrayOfStrings(approvalSpec.risk_classes)) {
    pushIssue(issues, "INVALID_RISK_CLASS_SET", specPath, "risk_classes MUST be a non-empty string array.");
  }
  if (!ensureArrayOfStrings(approvalSpec.approval_modes)) {
    pushIssue(issues, "INVALID_APPROVAL_MODE_SET", specPath, "approval_modes MUST be a non-empty string array.");
  }
  if (!ensureArrayOfStrings(approvalSpec.grant_scopes)) {
    pushIssue(issues, "INVALID_GRANT_SCOPE_SET", specPath, "grant_scopes MUST be a non-empty string array.");
  }
  const commandClassIds = Object.keys(approvalSpec.command_classes);
  const fileClassRuleIds = Object.keys(approvalSpec.file_classes);
  const knownFileClasses = new Set(fileClassIds);
  for (const [commandClass, entry] of Object.entries(approvalSpec.command_classes)) {
    validateApprovalRuleEntry(issues, specPath, `command_classes.${commandClass}`, entry);
  }
  for (const [fileClass, entry] of Object.entries(approvalSpec.file_classes)) {
    if (!knownFileClasses.has(fileClass)) {
      pushIssue(issues, "INVALID_FILE_CLASS_REF", specPath, `approval matrix references unknown file class ${fileClass}.`);
    }
    validateApprovalRuleEntry(issues, specPath, `file_classes.${fileClass}`, entry);
  }
  if (isObject(approvalSpec.command_class_aliases)) {
    for (const target of Object.values(approvalSpec.command_class_aliases)) {
      if (!commandClassIds.includes(target)) {
        pushIssue(issues, "INVALID_COMMAND_CLASS_REF", specPath, `command_class_aliases target ${target} is not defined.`);
      }
    }
  }
  if (isObject(approvalSpec.notify_only)) {
    for (const field of ["allowed_change_classes", "forbidden_command_classes", "forbidden_file_classes"]) {
      if (!ensureArrayOfStrings(approvalSpec.notify_only[field])) {
        pushIssue(issues, "INVALID_NOTIFY_ONLY_RULE", specPath, `notify_only.${field} MUST be a non-empty string array.`);
      }
    }
  }
  return { commandClassIds, fileClassRuleIds };
}

function validateHookSpec(hookSpec, issues) {
  const specPath = "workflow/hooks.spec.yaml";
  if (!hookSpec || !Array.isArray(hookSpec.hook_matrix)) {
    pushIssue(issues, "INVALID_HOOK_SPEC", specPath, "hook_matrix MUST be a non-empty array.");
    return;
  }
  if (!isObject(hookSpec.contract)) {
    pushIssue(issues, "INVALID_HOOK_CONTRACT", specPath, "contract MUST be a mapping.");
  } else {
    if (typeof hookSpec.contract.stdout_json_only !== "boolean") {
      pushIssue(issues, "INVALID_HOOK_CONTRACT", specPath, "contract.stdout_json_only MUST be boolean.");
    }
    if (!ensureArrayOfStrings(hookSpec.contract.verdicts)) {
      pushIssue(issues, "INVALID_HOOK_CONTRACT", specPath, "contract.verdicts MUST be a non-empty string array.");
    } else {
      for (const verdict of hookSpec.contract.verdicts) {
        if (!HOOK_VERDICTS.has(verdict)) {
          pushIssue(issues, "INVALID_HOOK_VERDICT", specPath, `Unknown verdict ${verdict}.`);
        }
      }
    }
  }
  if (!Array.isArray(hookSpec.event_matrix) || hookSpec.event_matrix.length === 0) {
    pushIssue(issues, "INVALID_HOOK_EVENT_MATRIX", specPath, "event_matrix MUST be a non-empty array.");
  } else {
    for (const [index, event] of hookSpec.event_matrix.entries()) {
      if (!isObject(event)) {
        pushIssue(issues, "INVALID_HOOK_EVENT", specPath, `event_matrix[${index}] MUST be a mapping.`);
        continue;
      }
      if (typeof event.event !== "string" || event.event.length === 0) {
        pushIssue(issues, "INVALID_HOOK_EVENT", specPath, `event_matrix[${index}] event MUST be a non-empty string.`);
      }
      if (typeof event.required !== "boolean") {
        pushIssue(issues, "INVALID_HOOK_EVENT", specPath, `event_matrix[${index}] required MUST be a boolean.`);
      }
      if (!isObject(event.engines)) {
        pushIssue(issues, "INVALID_HOOK_EVENT", specPath, `event_matrix[${index}] engines MUST be a mapping.`);
      } else {
        for (const [engine, value] of Object.entries(event.engines)) {
          if (!ENGINE_KINDS.has(engine)) {
            pushIssue(issues, "INVALID_ENGINE_KIND", specPath, `event_matrix[${index}] unknown engine ${engine}.`);
          }
          if (!HOOK_AVAILABILITY.has(value)) {
            pushIssue(issues, "INVALID_HOOK_EVENT", specPath, `event_matrix[${index}] ${engine} availability invalid.`);
          }
        }
      }
    }
  }
  if (hookSpec.hook_matrix.length === 0) {
    pushIssue(issues, "EMPTY_HOOK_MATRIX", specPath, "hook_matrix MUST define at least one hook.");
  }
  for (const [index, hook] of hookSpec.hook_matrix.entries()) {
    if (!isObject(hook)) {
      pushIssue(issues, "INVALID_HOOK_ENTRY", specPath, `hook_matrix[${index}] MUST be a mapping.`);
      continue;
    }
    if (typeof hook.event !== "string" || hook.event.length === 0) {
      pushIssue(issues, "INVALID_HOOK_EVENT", specPath, `hook_matrix[${index}] event MUST be a non-empty string.`);
    }
    if (typeof hook.script !== "string" || hook.script.length === 0) {
      pushIssue(issues, "INVALID_HOOK_SCRIPT", specPath, `hook_matrix[${index}] script MUST be a non-empty string.`);
    }
    if (typeof hook.purpose !== "string" || hook.purpose.length === 0) {
      pushIssue(issues, "INVALID_HOOK_PURPOSE", specPath, `hook_matrix[${index}] purpose MUST be a non-empty string.`);
    }
    if (typeof hook.required !== "boolean") {
      pushIssue(issues, "INVALID_HOOK_REQUIRED", specPath, `hook_matrix[${index}] required MUST be a boolean.`);
    }
  }

  const classification = hookSpec.command_classification;
  if (!classification) {
    return;
  }
  if (!ensureArrayOfStrings(classification.priority)) {
    pushIssue(issues, "INVALID_COMMAND_PRIORITY", specPath, "command_classification.priority MUST be a non-empty string array.");
  }
  if (!isObject(classification.classes)) {
    pushIssue(issues, "INVALID_COMMAND_CLASSES", specPath, "command_classification.classes MUST be a mapping.");
    return;
  }
  for (const [classId, entry] of Object.entries(classification.classes)) {
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_COMMAND_CLASS_ENTRY", specPath, `command_classification.classes.${classId} MUST be a mapping.`);
      continue;
    }
    if (!Array.isArray(entry.patterns) || entry.patterns.length === 0) {
      pushIssue(issues, "INVALID_COMMAND_CLASS_PATTERNS", specPath, `command_classification.classes.${classId}.patterns MUST be a non-empty array.`);
      continue;
    }
    for (const [patternIndex, patternEntry] of entry.patterns.entries()) {
      if (!isObject(patternEntry) || typeof patternEntry.regex !== "string" || patternEntry.regex.length === 0) {
        pushIssue(issues, "INVALID_COMMAND_PATTERN", specPath, `command_classification.classes.${classId}.patterns[${patternIndex}] regex MUST be a non-empty string.`);
      }
    }
  }
}

function validateHookContractSpec(hookContractSpec, issues) {
  const specPath = "workflow/hook-contract.schema.yaml";
  if (!hookContractSpec || !isObject(hookContractSpec.contract)) {
    pushIssue(issues, "INVALID_HOOK_CONTRACT_SPEC", specPath, "contract MUST be a mapping.");
    return;
  }
  const contract = hookContractSpec.contract;
  if (typeof contract.stdout_json_only !== "boolean") {
    pushIssue(issues, "INVALID_HOOK_CONTRACT_SPEC", specPath, "stdout_json_only MUST be boolean.");
  }
  if (typeof contract.stdin_single_read !== "boolean") {
    pushIssue(issues, "INVALID_HOOK_CONTRACT_SPEC", specPath, "stdin_single_read MUST be boolean.");
  }
  if (!ensureArrayOfStrings(contract.verdict_enum)) {
    pushIssue(issues, "INVALID_HOOK_CONTRACT_SPEC", specPath, "verdict_enum MUST be a non-empty string array.");
  } else {
    for (const verdict of contract.verdict_enum) {
      if (!HOOK_VERDICTS.has(verdict)) {
        pushIssue(issues, "INVALID_HOOK_VERDICT", specPath, `Unknown verdict ${verdict}.`);
      }
    }
  }
  if (!isObject(hookContractSpec.engine_translation)) {
    pushIssue(issues, "INVALID_HOOK_CONTRACT_SPEC", specPath, "engine_translation MUST be a mapping.");
  }
}

function validateOutputTemplatesSpec(outputTemplatesSpec, issues) {
  const specPath = "workflow/output-templates.spec.yaml";
  if (!outputTemplatesSpec || !isObject(outputTemplatesSpec.templates)) {
    pushIssue(issues, "INVALID_OUTPUT_TEMPLATES", specPath, "templates MUST be a mapping.");
    return [];
  }
  const templateIds = Object.keys(outputTemplatesSpec.templates);
  if (templateIds.length === 0) {
    pushIssue(issues, "EMPTY_OUTPUT_TEMPLATES", specPath, "At least one output template MUST be defined.");
  }
  for (const templateId of templateIds) {
    const entry = outputTemplatesSpec.templates[templateId];
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_ENTRY", specPath, `${templateId} MUST be a mapping.`);
      continue;
    }
    if (typeof entry.title !== "string" || entry.title.length === 0) {
      pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_ENTRY", specPath, `${templateId} title MUST be a non-empty string.`);
    }
    if (typeof entry.output_level !== "string" || entry.output_level.length === 0) {
      pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_ENTRY", specPath, `${templateId} output_level MUST be a non-empty string.`);
    }
    if (typeof entry.i18n_key !== "string" || entry.i18n_key.length === 0) {
      pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_ENTRY", specPath, `${templateId} i18n_key MUST be a non-empty string.`);
    }
    if (!ensureArrayOfStrings(entry.required_variables)) {
      pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_ENTRY", specPath, `${templateId} required_variables MUST be a non-empty string array.`);
    }
  }
  return templateIds;
}

function validateSkillsSpec(skillsSpec, rootDir, capabilityIds, outputTemplateIds, issues) {
  const specPath = "workflow/skills.spec.yaml";
  if (!skillsSpec || !isObject(skillsSpec.skills)) {
    pushIssue(issues, "INVALID_SKILLS_SPEC", specPath, "skills MUST be a mapping.");
    return [];
  }
  if (!isObject(skillsSpec.registry)) {
    pushIssue(issues, "INVALID_SKILLS_REGISTRY", specPath, "registry MUST be a mapping.");
  } else {
    if (typeof skillsSpec.registry.revision !== "string" || skillsSpec.registry.revision.length === 0) {
      pushIssue(issues, "INVALID_SKILLS_REGISTRY", specPath, "registry.revision MUST be a non-empty string.");
    }
  }
  const defaults = isObject(skillsSpec.defaults) ? skillsSpec.defaults : {};
  const defaultOutputs = toArray(defaults.output_templates);
  const outputTemplateSet = new Set(outputTemplateIds);
  for (const templateId of defaultOutputs) {
    if (!outputTemplateSet.has(templateId)) {
      pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_REF", specPath, `defaults.output_templates references ${templateId} not in registry.`);
    }
  }

  const capabilitySet = new Set(capabilityIds);
  const skillIds = Object.keys(skillsSpec.skills);
  if (skillIds.length === 0) {
    pushIssue(issues, "EMPTY_SKILLS_REGISTRY", specPath, "At least one skill MUST be defined.");
  }
  for (const skillId of skillIds) {
    const entry = skillsSpec.skills[skillId];
    if (!isObject(entry)) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} MUST be a mapping.`);
      continue;
    }
    if (typeof entry.title !== "string" || entry.title.length === 0) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} title MUST be a non-empty string.`);
    }
    if (typeof entry.summary !== "string" || entry.summary.length === 0) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} summary MUST be a non-empty string.`);
    }
    if (typeof entry.category !== "string" || entry.category.length === 0) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} category MUST be a non-empty string.`);
    }
    if (typeof entry.entry !== "string" || entry.entry.length === 0) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} entry MUST be a non-empty string.`);
    } else {
      const skillPath = path.join(rootDir, entry.entry);
      if (!fs.existsSync(skillPath)) {
        pushIssue(issues, "MISSING_SKILL_ENTRY", specPath, `${skillId} entry ${entry.entry} does not exist.`);
      }
    }
    if (!ensureArrayOfStrings(entry.capabilities)) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} capabilities MUST be a non-empty string array.`);
    } else {
      for (const capabilityId of entry.capabilities) {
        if (!capabilitySet.has(capabilityId)) {
          pushIssue(issues, "INVALID_CAPABILITY_REF", specPath, `${skillId} references unknown capability ${capabilityId}.`);
        }
      }
    }
    if (entry.phases) {
      if (!ensureArrayOfStrings(entry.phases)) {
        pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} phases MUST be a non-empty string array.`);
      } else {
        for (const phase of entry.phases) {
          if (!PHASE_IDS.has(phase)) {
            pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} phase ${phase} is invalid.`);
          }
        }
      }
    }
    if (entry.triggers) {
      if (!isObject(entry.triggers)) {
        pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} triggers MUST be a mapping.`);
      } else if (entry.triggers.mode && !TRIGGER_MODES.has(entry.triggers.mode)) {
        pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} triggers.mode invalid.`);
      }
    }
    if (entry.policy) {
      if (!isObject(entry.policy)) {
        pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} policy MUST be a mapping.`);
      } else {
        if (typeof entry.policy.disable_model_invocation !== "boolean") {
          pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} policy.disable_model_invocation MUST be boolean.`);
        }
        if (entry.policy.allowed_tools && !ensureArrayOfStrings(entry.policy.allowed_tools)) {
          pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} policy.allowed_tools MUST be a string array.`);
        }
      }
    }
    const templateRefs = entry.output_templates || defaultOutputs;
    if (templateRefs) {
      if (!ensureArrayOfStrings(templateRefs)) {
        pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} output_templates MUST be a string array.`);
      } else {
        for (const templateId of templateRefs) {
          if (!outputTemplateSet.has(templateId)) {
            pushIssue(issues, "INVALID_OUTPUT_TEMPLATE_REF", specPath, `${skillId} output_templates references ${templateId}.`);
          }
        }
      }
    }
    if (entry.parent_skill && !skillIds.includes(entry.parent_skill)) {
      pushIssue(issues, "INVALID_SKILL_ENTRY", specPath, `${skillId} parent_skill ${entry.parent_skill} not found.`);
    }
  }
  return skillIds;
}

function validateExecutionSpec(spec, specPath, issues) {
  if (!spec || !isObject(spec)) {
    return;
  }
  const actionTypes = spec.node_action_schema?.action_types;
  if (!Array.isArray(actionTypes) || actionTypes.length === 0) {
    pushIssue(issues, "INVALID_EXECUTION_ACTION_TYPES", specPath, "node_action_schema.action_types MUST be a non-empty array.");
  }
  const split3 = spec.execute_3split;
  if (!isObject(split3)) {
    pushIssue(issues, "MISSING_EXECUTE_3SPLIT", specPath, "execute_3split is required.");
  } else {
    const phases = split3.phases;
    const EXPECTED_PHASES = ["red", "implementation", "verify"];
    if (!Array.isArray(phases) || EXPECTED_PHASES.some((p, i) => phases[i] !== p)) {
      pushIssue(issues, "INVALID_EXECUTE_3SPLIT_PHASES", specPath, `execute_3split.phases MUST be [${EXPECTED_PHASES.join(", ")}] in order.`);
    }
  }
  const constraints = spec.persona_action_constraints;
  if (!isObject(constraints)) {
    pushIssue(issues, "MISSING_PERSONA_ACTION_CONSTRAINTS", specPath, "persona_action_constraints is required.");
  } else {
    for (const [personaId, entry] of Object.entries(constraints)) {
      if (!isObject(entry)) {
        pushIssue(issues, "INVALID_PERSONA_CONSTRAINT_ENTRY", specPath, `persona_action_constraints.${personaId} MUST be a mapping.`);
        continue;
      }
      if (typeof entry.may_write !== "boolean") {
        pushIssue(issues, "MISSING_PERSONA_MAY_WRITE", specPath, `persona_action_constraints.${personaId} MUST define may_write as boolean.`);
      }
      if (typeof entry.may_run !== "boolean") {
        pushIssue(issues, "MISSING_PERSONA_MAY_RUN", specPath, `persona_action_constraints.${personaId} MUST define may_run as boolean.`);
      }
    }
  }
}

function validateSkillTiers(spec, specPath, issues) {
  if (!spec || !isObject(spec)) {
    return;
  }
  const skillTiers = spec.skill_tiers;
  if (!isObject(skillTiers)) {
    pushIssue(issues, "MISSING_SKILL_TIERS", specPath, "skill_tiers MUST be defined with core and auxiliary entries.", "warning");
    return;
  }
  for (const tier of ["core", "auxiliary"]) {
    if (!isObject(skillTiers[tier])) {
      pushIssue(issues, "MISSING_SKILL_TIER_ENTRY", specPath, `skill_tiers.${tier} MUST be defined.`, "warning");
    }
  }
  const registry = spec.skills;
  if (!isObject(registry)) {
    return;
  }
  for (const [skillId, entry] of Object.entries(registry)) {
    if (!isObject(entry)) {
      continue;
    }
    if (!entry.tier) {
      pushIssue(issues, "MISSING_SKILL_TIER", specPath, `skill ${skillId} does not define a tier field.`, "warning");
    } else if (!["core", "auxiliary"].includes(entry.tier)) {
      pushIssue(issues, "INVALID_SKILL_TIER", specPath, `skill ${skillId} tier must be core or auxiliary, got ${entry.tier}.`);
    }
  }
}

function validateCrossReferences(specs, capabilityIds, personaIds, fileClassIds, approvalMatrix, issues) {
  const capabilitySet = new Set(capabilityIds);
  const personaSet = new Set(personaIds);
  const fileClassSet = new Set(fileClassIds);
  const approvalCommandSet = new Set(approvalMatrix.commandClassIds);
  const approvalFileSet = new Set(approvalMatrix.fileClassRuleIds);
  const capabilitiesSpec = specs.get("workflow/capabilities.yaml");
  const personaSpec = specs.get("workflow/persona-bindings.yaml");
  const routerSpec = specs.get("workflow/router.spec.yaml");
  const runtimeSpec = specs.get("workflow/runtime.schema.yaml");
  const policySpec = specs.get("workflow/policy.spec.yaml");

  if (capabilitiesSpec && isObject(capabilitiesSpec.capabilities) && personaSpec && isObject(personaSpec.personas)) {
    for (const [capabilityId, capability] of Object.entries(capabilitiesSpec.capabilities)) {
      if (!personaSet.has(capability.default_persona)) {
        pushIssue(issues, "INVALID_PERSONA_REF", "workflow/capabilities.yaml", `${capabilityId} default_persona ${capability.default_persona} is not defined.`);
      }
      for (const personaId of toArray(capability.allowed_personas)) {
        if (!personaSet.has(personaId)) {
          pushIssue(issues, "INVALID_PERSONA_REF", "workflow/capabilities.yaml", `${capabilityId} allowed_personas references unknown persona ${personaId}.`);
        }
      }
      const defaultPersona = personaSpec.personas[capability.default_persona];
      if (defaultPersona && Array.isArray(defaultPersona.allowed_capabilities) && !defaultPersona.allowed_capabilities.includes(capabilityId)) {
        pushIssue(issues, "INVALID_CAPABILITY_REF", "workflow/persona-bindings.yaml", `${capability.default_persona} must allow capability ${capabilityId}.`);
      }
    }
  }

  if (routerSpec && isObject(routerSpec.persona_capability_routing)) {
    for (const personaId of toArray(routerSpec.persona_capability_routing.personas)) {
      if (!personaSet.has(personaId)) {
        pushIssue(issues, "INVALID_PERSONA_REF", "workflow/router.spec.yaml", `router persona ${personaId} is not defined in persona-bindings.`);
      }
    }
    for (const binding of toArray(routerSpec.persona_capability_routing.default_bindings)) {
      if (!personaSet.has(binding.next_persona)) {
        pushIssue(issues, "INVALID_PERSONA_REF", "workflow/router.spec.yaml", `router binding references unknown persona ${binding.next_persona}.`);
      }
      if (!capabilitySet.has(binding.next_capability)) {
        pushIssue(issues, "INVALID_CAPABILITY_REF", "workflow/router.spec.yaml", `router binding references unknown capability ${binding.next_capability}.`);
      }
    }
    for (const personaId of toArray(routerSpec.persona_capability_routing.review_chain)) {
      if (!personaSet.has(personaId)) {
        pushIssue(issues, "INVALID_PERSONA_REF", "workflow/router.spec.yaml", `router review_chain references unknown persona ${personaId}.`);
      }
    }
  }

  if (routerSpec) {
    if (routerSpec.phase_routing?.execution_mode !== "single_active_phase") {
      pushIssue(issues, "INVALID_EXECUTION_MODE", "workflow/router.spec.yaml", "phase_routing.execution_mode MUST remain single_active_phase in V1.");
    }
    if (routerSpec.phase_routing?.parallel_phases_supported !== false) {
      pushIssue(issues, "PARALLEL_PHASES_NOT_SUPPORTED_V1", "workflow/router.spec.yaml", "parallel_phases_supported MUST remain false in V1.");
    }
    if (routerSpec.node_routing?.execution_mode !== "single_active_node") {
      pushIssue(issues, "INVALID_EXECUTION_MODE", "workflow/router.spec.yaml", "node_routing.execution_mode MUST remain single_active_node in V1.");
    }
    if (routerSpec.node_routing?.parallel_nodes_supported !== false) {
      pushIssue(issues, "PARALLEL_NODES_NOT_SUPPORTED_V1", "workflow/router.spec.yaml", "parallel_nodes_supported MUST remain false in V1.");
    }
  }

  if (runtimeSpec) {
    const enumValues = runtimeSpec.session?.properties?.node?.properties?.owner_persona?.enum;
    if (Array.isArray(enumValues)) {
      for (const personaId of enumValues) {
        if (!personaSet.has(personaId)) {
          pushIssue(issues, "INVALID_PERSONA_REF", "workflow/runtime.schema.yaml", `runtime owner_persona enum contains unknown persona ${personaId}.`);
        }
      }
    }
  }

  if (policySpec) {
    for (const commandClass of toArray(policySpec.approval_policy?.approval_required_for?.command_classes)) {
      if (!approvalCommandSet.has(commandClass)) {
        pushIssue(issues, "INVALID_COMMAND_CLASS_REF", "workflow/policy.spec.yaml", `policy references unknown command class ${commandClass}.`);
      }
    }
    for (const fileClass of toArray(policySpec.approval_policy?.approval_required_for?.file_classes)) {
      if (!fileClassSet.has(fileClass)) {
        pushIssue(issues, "INVALID_FILE_CLASS_REF", "workflow/policy.spec.yaml", `policy references unknown file class ${fileClass}.`);
      }
      if (!approvalFileSet.has(fileClass)) {
        pushIssue(issues, "INVALID_FILE_CLASS_REF", "workflow/approval-matrix.yaml", `approval matrix missing rule for file class ${fileClass}.`);
      }
    }
  }
}

function validateWorkflowSpecs(options = {}) {
  const rootDir = path.resolve(options.rootDir || path.join(__dirname, "..", ".."));
  const requestedSpecs = Array.isArray(options.specPaths) ? options.specPaths : [];
  const validateAll = Boolean(options.validateAll);
  const validateManifestOnly = Boolean(options.validateManifestOnly);
  const freezeGate = typeof options.freezeGate === "string" ? options.freezeGate : null;
  const validateScope = String(options.validateScope || "full").toLowerCase();

  const issues = [];
  const manifest = loadManifest(rootDir);
  const manifestEntries = validateManifest(manifest, issues);

  if (validateManifestOnly) {
    const hasError = issues.some((issue) => issue.severity !== "warning");
    return {
      ok: !hasError,
      issues,
      specsValidated: manifest ? [MANIFEST_PATH] : [],
      rootDir,
    };
  }

  const specPaths = collectRequestedSpecs(requestedSpecs, validateAll, manifestEntries);
  const specs = loadSpecs(rootDir, specPaths);

  if (manifestEntries) {
    const workflowSpecs = listWorkflowSpecs(rootDir);
    for (const specPath of workflowSpecs) {
      if (!manifestEntries.has(specPath)) {
        pushIssue(issues, "SPEC_NOT_REGISTERED", specPath, "Spec is not registered in validate-manifest.", "warning");
      }
    }
  }

  validateEnvelope(specs, manifestEntries, issues, freezeGate);

  if (validateScope === "envelope") {
    // Envelope-only validation (schema metadata + freeze gate), no deep spec checks.
  } else if (validateScope === "targeted" && requestedSpecs.length > 0) {
    const targetSet = new Set(requestedSpecs.map(normalizeSpecPath));
    for (const [specPath, spec] of specs.entries()) {
      if (!targetSet.has(specPath)) {
        continue;
      }
      if (specPath === "workflow/capabilities.yaml") {
        validateCapabilities(spec, issues);
        continue;
      }
      if (specPath === "workflow/persona-bindings.yaml") {
        validatePersonaBindings(spec, [], issues);
        continue;
      }
      if (specPath === "workflow/file-classes.yaml") {
        validateFileClasses(spec, issues);
        continue;
      }
      if (specPath === "workflow/approval-matrix.yaml") {
        validateApprovalMatrix(spec, [], issues);
        continue;
      }
      if (specPath === "workflow/hooks.spec.yaml") {
        validateHookSpec(spec, issues);
        continue;
      }
      if (specPath === "workflow/hook-contract.schema.yaml") {
        validateHookContractSpec(spec, issues);
        continue;
      }
      if (specPath === "workflow/output-templates.spec.yaml") {
        validateOutputTemplatesSpec(spec, issues);
        continue;
      }
      if (specPath === "workflow/skills.spec.yaml") {
        validateSkillsSpec(spec, rootDir, [], [], issues);
        continue;
      }
    }
  } else {
    const capabilityIds = validateCapabilities(specs.get("workflow/capabilities.yaml"), issues);
    const personaIds = validatePersonaBindings(specs.get("workflow/persona-bindings.yaml"), capabilityIds, issues);
    const fileClassIds = validateFileClasses(specs.get("workflow/file-classes.yaml"), issues);
    const approvalMatrix = validateApprovalMatrix(specs.get("workflow/approval-matrix.yaml"), fileClassIds, issues);
    validateHookSpec(specs.get("workflow/hooks.spec.yaml"), issues);
    validateHookContractSpec(specs.get("workflow/hook-contract.schema.yaml"), issues);
    const outputTemplateIds = validateOutputTemplatesSpec(specs.get("workflow/output-templates.spec.yaml"), issues);
    validateSkillsSpec(specs.get("workflow/skills.spec.yaml"), rootDir, capabilityIds, outputTemplateIds, issues);
    validateSkillTiers(specs.get("workflow/skills.spec.yaml"), "workflow/skills.spec.yaml", issues);
    validateExecutionSpec(specs.get("workflow/execution.spec.yaml"), "workflow/execution.spec.yaml", issues);
    validateCrossReferences(specs, capabilityIds, personaIds, fileClassIds, approvalMatrix, issues);
  }

  const hasError = issues.some((issue) => issue.severity !== "warning");

  return {
    ok: !hasError,
    issues,
    specsValidated: [...specs.keys()],
    rootDir,
  };
}

module.exports = {
  DEFAULT_SPEC_PATHS,
  validateWorkflowSpecs,
};
