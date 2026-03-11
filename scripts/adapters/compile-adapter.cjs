#!/usr/bin/env node
"use strict";

const crypto = require("node:crypto");
const path = require("node:path");

const { validateWorkflowSpecs } = require("../runtime/spec-validator.cjs");
const { loadWorkflowSpecs } = require("../runtime/workflow-specs.cjs");

const ENGINE_PROFILES = {
  claude_code: {
    instruction_file: "CLAUDE.md",
    config_files: [".claude/settings.json"],
    native_hook_support: true,
    hook_strategy: "native_settings_hooks",
    context_strategy: "project_instruction_file",
    hierarchy_mode: "root_and_nested_project_context",
    approval_surface: "hook_block_and_human_prompt",
    references: [
      "refer/agent-source-code/claude-code-main/README.md",
      "https://code.claude.com/docs/en/memory",
      "https://docs.anthropic.com/en/docs/claude-code/hooks",
    ],
  },
  codex: {
    instruction_file: "AGENTS.md",
    config_files: [".codex/config.toml"],
    native_hook_support: false,
    hook_strategy: "instruction_and_sandbox_bridge",
    context_strategy: "hierarchical_agents_md",
    hierarchy_mode: "root_to_nested_agents_scope",
    approval_surface: "approval_policy_and_sandbox",
    references: [
      "refer/agent-source-code/codex-main/docs/agents_md.md",
      "refer/agent-source-code/codex-main/docs/config.md",
      "https://developers.openai.com/codex/guides/agents-md",
    ],
  },
  gemini_cli: {
    instruction_file: "GEMINI.md",
    config_files: [".gemini/settings.json"],
    native_hook_support: true,
    hook_strategy: "native_settings_hooks",
    context_strategy: "hierarchical_and_jit_context_files",
    hierarchy_mode: "global_workspace_and_jit_context",
    approval_surface: "settings_hooks_and_approval_mode",
    references: [
      "refer/agent-source-code/gemini-cli-main/docs/cli/gemini-md.md",
      "refer/agent-source-code/gemini-cli-main/docs/hooks/reference.md",
      "refer/agent-source-code/gemini-cli-main/docs/cli/settings.md",
      "refer/agent-source-code/gemini-cli-main/GEMINI.md",
      "https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/gemini-md.md",
      "https://geminicli.com/docs/hooks",
    ],
  },
};

const DEFAULT_HOOK_CONTRACTS = [
  { event: "SessionStart", script: "scripts/hooks/sy-session-start.cjs", purpose: "bootstrap routing and constraints" },
  { event: "UserPromptSubmit", script: "scripts/hooks/sy-prompt-refresh.cjs", purpose: "long-session prompt re-anchor" },
  { event: "PreToolUse:Bash", script: "scripts/hooks/sy-pretool-bash.cjs", purpose: "destructive command and git guard" },
  { event: "PreToolUse:Bash", script: "scripts/hooks/sy-pretool-bash-budget.cjs", purpose: "loop budget gate" },
  { event: "PreToolUse:Write|Edit", script: "scripts/hooks/sy-pretool-write.cjs", purpose: "TDD, secret, protected-file, debug gates" },
  { event: "PreToolUse:Write|Edit", script: "scripts/hooks/sy-pretool-write-session.cjs", purpose: "session integrity gate" },
  { event: "PostToolUse:Write|Edit", script: "scripts/hooks/sy-posttool-write.cjs", purpose: "write evidence and scope drift capture" },
  { event: "PostToolUse:Bash", script: "scripts/hooks/sy-posttool-bash-verify.cjs", purpose: "verification evidence capture" },
  { event: "Stop", script: "scripts/hooks/sy-stop.cjs", purpose: "checkpoint and resume frontier gate" },
];

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function toArray(value) {
  return Array.isArray(value) ? value : [];
}

function uniq(list) {
  return Array.from(new Set(toArray(list).filter(Boolean)));
}

function mergeDefaults(defaultValue, overrideValue) {
  const base = isObject(defaultValue) ? defaultValue : {};
  const override = isObject(overrideValue) ? overrideValue : {};
  const merged = { ...base, ...override };
  for (const [key, value] of Object.entries(override)) {
    if (Array.isArray(base[key]) && Array.isArray(value)) {
      merged[key] = uniq([...base[key], ...value]);
    } else if (isObject(base[key]) && isObject(value)) {
      merged[key] = mergeDefaults(base[key], value);
    }
  }
  return merged;
}

function getEngineProfile(engine) {
  const profile = ENGINE_PROFILES[engine];
  if (!profile) {
    throw new Error(`Unsupported engine: ${engine}`);
  }
  return profile;
}

function ensureValidSpecs(rootDir) {
  const validation = validateWorkflowSpecs({ rootDir, validateAll: true });
  if (!validation.ok) {
    const summary = validation.issues.map((issue) => `${issue.code}:${issue.specPath}:${issue.message}`).join(" | ");
    throw new Error(`COMPILER_SPEC_VALIDATION_FAIL ${summary}`);
  }
}

function buildCapabilityBindings(specs, engine) {
  const capabilities = isObject(specs.capabilities?.capabilities) ? specs.capabilities.capabilities : {};
  return Object.entries(capabilities)
    .filter(([, capability]) => toArray(capability.engine_support).includes(engine))
    .map(([capabilityId, capability]) => ({
      capability_id: capabilityId,
      category: capability.category,
      default_persona: capability.default_persona,
      allowed_personas: toArray(capability.allowed_personas),
      writes_files: Boolean(capability.writes_files),
      runs_commands: Boolean(capability.runs_commands),
      produces_review_verdict: Boolean(capability.produces_review_verdict),
      requires_human: capability.requires_human === true,
    }));
}

function buildSkillRegistry(specs) {
  const skillsSpec = specs.skillsSpec || {};
  const registryMeta = isObject(skillsSpec.registry) ? skillsSpec.registry : {};
  const defaults = isObject(skillsSpec.defaults) ? skillsSpec.defaults : {};
  const defaultPhases = toArray(defaults.phases);
  const defaultTriggers = isObject(defaults.triggers) ? defaults.triggers : {};
  const defaultArguments = isObject(defaults.arguments) ? defaults.arguments : {};
  const defaultPolicy = isObject(defaults.policy) ? defaults.policy : {};
  const defaultTemplates = toArray(defaults.output_templates);
  const skills = isObject(skillsSpec.skills) ? skillsSpec.skills : {};

  const entries = Object.entries(skills).map(([skillId, skill]) => {
    const skillObj = isObject(skill) ? skill : {};
    const triggers = mergeDefaults(defaultTriggers, skillObj.triggers);
    const argumentsConfig = mergeDefaults(defaultArguments, skillObj.arguments);
    const policyConfig = mergeDefaults(defaultPolicy, skillObj.policy);
    const outputTemplates = uniq([...defaultTemplates, ...toArray(skillObj.output_templates)]);
    const phases = uniq([...defaultPhases, ...toArray(skillObj.phases)]);
    const invocationPolicy = String(triggers.mode || defaultTriggers.mode || "explicit");

    return {
      skill_id: skillId,
      title: skillObj.title || skillObj.name || skillId,
      summary: skillObj.summary || "",
      category: skillObj.category || "general",
      entry: skillObj.entry || "",
      parent_skill: skillObj.parent_skill || null,
      capabilities: toArray(skillObj.capabilities),
      phases,
      triggers: {
        mode: invocationPolicy,
        keywords: uniq([...toArray(triggers.keywords), ...toArray(skillObj?.triggers?.keywords)]),
      },
      arguments: {
        schema: isObject(argumentsConfig.schema) ? argumentsConfig.schema : {},
        argument_hints: toArray(argumentsConfig.argument_hints),
      },
      policy: {
        disable_model_invocation: policyConfig.disable_model_invocation === true,
        allowed_tools: toArray(policyConfig.allowed_tools),
      },
      output_templates: outputTemplates,
    };
  });

  const hashSeed = {
    defaults: {
      phases: defaultPhases,
      triggers: defaultTriggers,
      arguments: defaultArguments,
      policy: defaultPolicy,
      output_templates: defaultTemplates,
    },
    skills: entries,
  };
  const computedHash = crypto.createHash("sha256")
    .update(JSON.stringify(hashSeed))
    .digest("hex");
  const resolvedHash = registryMeta.spec_hash && registryMeta.spec_hash !== "pending"
    ? registryMeta.spec_hash
    : computedHash;

  return {
    registry_revision: registryMeta.revision || "",
    spec_hash: resolvedHash,
    change_detection: registryMeta.change_detection || {},
    skills: entries,
  };
}

function buildSkillStubs(skillRegistry) {
  const skills = Array.isArray(skillRegistry.skills) ? skillRegistry.skills : [];
  return skills.map((skill) => ({
    skill_id: skill.skill_id,
    title: skill.title,
    summary: skill.summary,
    category: skill.category,
    entry: skill.entry,
    invocation_policy: skill.triggers?.mode || "explicit",
    phases: skill.phases,
    output_templates: skill.output_templates,
    argument_hints: skill.arguments?.argument_hints || [],
  }));
}

function buildPersonaBindings(specs, supportedCapabilityIds) {
  const supportedSet = new Set(supportedCapabilityIds);
  const personas = isObject(specs.personaBindings?.personas) ? specs.personaBindings.personas : {};
  return Object.entries(personas).map(([personaId, persona]) => ({
    persona_id: personaId,
    class: persona.class,
    allowed_capabilities: toArray(persona.allowed_capabilities).filter((capabilityId) => supportedSet.has(capabilityId)),
    may_write_files: Boolean(persona.may_write_files),
    may_run_commands: Boolean(persona.may_run_commands),
    input_contract: toArray(persona.input_contract),
    output_contract: toArray(persona.output_contract),
  }));
}

function ensureCapabilityCoverage(bundle) {
  const personaMap = new Map(bundle.persona_bindings.map((persona) => [persona.persona_id, persona]));
  for (const capability of bundle.capability_bindings) {
    const persona = personaMap.get(capability.default_persona);
    if (!persona || !persona.allowed_capabilities.includes(capability.capability_id)) {
      throw new Error(
        `COMPILER_MISSING_CAPABILITY_BINDING capability=${capability.capability_id} default_persona=${capability.default_persona}`,
      );
    }
  }
}

function buildApprovalContract(specs) {
  const approvalMatrix = specs.approvalMatrix || {};
  const policySpec = specs.policySpec || {};
  return {
    risk_classes: toArray(approvalMatrix.risk_classes),
    approval_modes: toArray(approvalMatrix.approval_modes),
    grant_scopes: toArray(approvalMatrix.grant_scopes),
    approval_required_for: {
      command_classes: toArray(policySpec.approval_policy?.approval_required_for?.command_classes),
      file_classes: toArray(policySpec.approval_policy?.approval_required_for?.file_classes),
    },
    notify_only: {
      enabled: approvalMatrix.notify_only?.allowed_change_classes?.length > 0,
      allowed_change_classes: toArray(approvalMatrix.notify_only?.allowed_change_classes),
      forbidden_command_classes: toArray(approvalMatrix.notify_only?.forbidden_command_classes),
      forbidden_file_classes: toArray(approvalMatrix.notify_only?.forbidden_file_classes),
    },
  };
}

function buildFileClassContract(specs) {
  const fileClasses = specs.fileClasses || {};
  const classes = isObject(fileClasses.classes) ? fileClasses.classes : {};
  return {
    match_precedence: toArray(fileClasses.match_precedence),
    classes: Object.entries(classes).map(([classId, entry]) => ({
      class_id: classId,
      default_risk_class: entry.default_risk_class,
      patterns: toArray(entry.patterns),
    })),
  };
}

function buildHookContract(engine, profile, hookSpec) {
  const hookMatrix = Array.isArray(hookSpec?.hook_matrix) && hookSpec.hook_matrix.length > 0
    ? hookSpec.hook_matrix
    : DEFAULT_HOOK_CONTRACTS;
  return {
    native_hook_support: profile.native_hook_support,
    hook_strategy: profile.hook_strategy,
    hooks: hookMatrix.map((hook) => ({
      event: hook.event,
      script: hook.script,
      purpose: hook.purpose,
      enforcement_mode: profile.native_hook_support ? "native" : "bridged",
      required:
        engine === "claude_code" || engine === "gemini_cli"
          ? Boolean(hook.required)
          : ["SessionStart", "Stop"].includes(hook.event) || hook.event.startsWith("PreToolUse"),
    })),
  };
}

function buildCapabilityGapReport(specs) {
  const hookSpec = specs.hookSpec || {};
  const eventMatrix = toArray(hookSpec.event_matrix);
  const engines = Object.keys(ENGINE_PROFILES);
  const events = eventMatrix.map((entry) => {
    const engineStatuses = {};
    for (const engine of engines) {
      const status = entry?.engines?.[engine] || "unknown";
      engineStatuses[engine] = status;
    }
    return {
      event: entry.event,
      purpose: entry.purpose,
      required: Boolean(entry.required),
      engine_status: engineStatuses,
      gap_engines: engines.filter((engine) => engineStatuses[engine] !== "supported"),
    };
  });

  const gapsByEngine = {};
  const requiredGapsByEngine = {};
  for (const engine of engines) {
    gapsByEngine[engine] = events
      .filter((entry) => entry.engine_status[engine] !== "supported")
      .map((entry) => entry.event);
    requiredGapsByEngine[engine] = events
      .filter((entry) => entry.required && entry.engine_status[engine] !== "supported")
      .map((entry) => entry.event);
  }

  return {
    schema_kind: "hook_capability_gap_report",
    schema_version: 1,
    generated_at: new Date().toISOString(),
    source: "workflow/hooks.spec.yaml",
    total_events: events.length,
    required_events: events.filter((entry) => entry.required).length,
    events,
    summary: {
      gaps_by_engine: gapsByEngine,
      required_gaps_by_engine: requiredGapsByEngine,
    },
  };
}

function buildRoutingPass(bundleBase, specs, engine) {
  const skillRegistry = buildSkillRegistry(specs);
  return {
    schema_kind: "adapter_routing_pass",
    schema_version: 1,
    schema_dialect: "seeyue_workflow_adapter/v1",
    engine,
    generated_at: new Date().toISOString(),
    render_targets: bundleBase.render_targets,
    language_policy: bundleBase.language_policy,
    router_bridge: bundleBase.router_bridge,
    review_chain: bundleBase.review_chain,
    skill_stubs: buildSkillStubs(skillRegistry),
  };
}

function buildSkillPass(bundleBase, specs, engine) {
  return {
    schema_kind: "adapter_skill_pass",
    schema_version: 1,
    schema_dialect: "seeyue_workflow_adapter/v1",
    engine,
    generated_at: new Date().toISOString(),
    render_targets: bundleBase.render_targets,
    skill_registry: buildSkillRegistry(specs),
    output_templates: specs.outputTemplates?.templates || {},
  };
}

function buildPolicyPass(bundleBase, specs, engine) {
  return {
    schema_kind: "adapter_policy_pass",
    schema_version: 1,
    schema_dialect: "seeyue_workflow_adapter/v1",
    engine,
    generated_at: new Date().toISOString(),
    engine_contract: bundleBase.engine_contract,
    approval_contract: bundleBase.approval_contract,
    file_class_contract: bundleBase.file_class_contract,
    hook_contract: bundleBase.hook_contract,
    capability_gap_report: bundleBase.capability_gap_report,
  };
}

function buildRouterBridge(specs) {
  const routing = specs.routerSpec?.persona_capability_routing || {};
  return {
    personas: toArray(routing.personas),
    default_bindings: toArray(routing.default_bindings).map((binding) => ({
      next_persona: binding.next_persona,
      next_capability: binding.next_capability,
      when: binding.when,
    })),
    review_chain: toArray(routing.review_chain),
  };
}

function buildLanguagePolicy() {
  return {
    agent_rule_language: "en",
    human_output_language: "zh-CN",
    approval_request_style: "short_explicit_action_oriented",
  };
}

function buildRenderTargets(profile) {
  return {
    instruction_file: profile.instruction_file,
    config_files: profile.config_files,
    skill_root: ".agents/skills",
    hook_root: "scripts/hooks",
  };
}

function compileAdapterBundle(options = {}) {
  const rootDir = path.resolve(options.rootDir || path.join(__dirname, "..", ".."));
  const engine = String(options.engine || "").trim();
  const profile = getEngineProfile(engine);

  ensureValidSpecs(rootDir);
  const specs = loadWorkflowSpecs(rootDir);
  const capabilityBindings = buildCapabilityBindings(specs, engine);
  const personaBindings = buildPersonaBindings(specs, capabilityBindings.map((entry) => entry.capability_id));

  const bundle = {
    schema_kind: "adapter_bundle",
    schema_version: 1,
    schema_dialect: "seeyue_workflow_adapter/v1",
    engine,
    generated_from_root: rootDir.replace(/\\/g, "/"),
    compiler: {
      name: "seeyue-compile-adapter",
      version: 1,
    },
    render_targets: buildRenderTargets(profile),
    language_policy: buildLanguagePolicy(),
    engine_contract: {
      native_hook_support: profile.native_hook_support,
      hook_strategy: profile.hook_strategy,
      context_strategy: profile.context_strategy,
      hierarchy_mode: profile.hierarchy_mode,
      approval_surface: profile.approval_surface,
      references: profile.references,
    },
    capability_bindings: capabilityBindings,
    persona_bindings: personaBindings,
    review_chain: toArray(specs.personaBindings?.review_chain),
    isolation_rules: toArray(specs.personaBindings?.isolation_rules),
    router_bridge: buildRouterBridge(specs),
    approval_contract: buildApprovalContract(specs),
    file_class_contract: buildFileClassContract(specs),
    hook_contract: buildHookContract(engine, profile, specs.hookSpec),
    capability_gap_report: buildCapabilityGapReport(specs),
  };

  bundle.passes = {
    routing: buildRoutingPass(bundle, specs, engine),
    skills: buildSkillPass(bundle, specs, engine),
    policy: buildPolicyPass(bundle, specs, engine),
  };

  ensureCapabilityCoverage(bundle);
  return bundle;
}

function compileAllAdapters(options = {}) {
  const engines = options.engines && options.engines.length > 0 ? options.engines : Object.keys(ENGINE_PROFILES);
  return engines.map((engine) => compileAdapterBundle({ ...options, engine }));
}

function parseArgs(argv) {
  const result = {
    rootDir: path.resolve(__dirname, "..", ".."),
    engine: null,
    pass: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--root") {
      index += 1;
      result.rootDir = path.resolve(argv[index]);
      continue;
    }
    if (token === "--engine") {
      index += 1;
      result.engine = argv[index];
      continue;
    }
    if (token === "--pass") {
      index += 1;
      result.pass = argv[index];
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
    const payload = args.engine
      ? compileAdapterBundle({ rootDir: args.rootDir, engine: args.engine })
      : compileAllAdapters({ rootDir: args.rootDir });

    if (args.pass && args.engine) {
      const passKey = String(args.pass || "").toLowerCase();
      const passMap = {
        routing: "routing",
        skills: "skills",
        skill: "skills",
        policy: "policy",
      };
      const resolved = passMap[passKey];
      if (!resolved || !payload.passes?.[resolved]) {
        throw new Error(`UNKNOWN_PASS ${args.pass}`);
      }
      process.stdout.write(`${JSON.stringify(payload.passes[resolved], null, 2)}\n`);
      return;
    }

    process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`);
  } catch (error) {
    console.error(`ADAPTER_COMPILER_FAIL ${error.message}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  ENGINE_PROFILES,
  DEFAULT_HOOK_CONTRACTS,
  buildSkillRegistry,
  compileAdapterBundle,
  compileAllAdapters,
};
