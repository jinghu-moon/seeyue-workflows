"use strict";

// interaction-builders.cjs — P1-N3: Request Builders
//
// Projects existing runtime states into proper InteractionRequest objects.
// Each builder produces a schema-v1 compliant request object.
//
// Interaction ID format: ix-YYYYMMDD-NNN
// Uses a module-level counter to ensure uniqueness within the same millisecond.

// ─── ID generation ───────────────────────────────────────────────────────────

let _counter = 0;

function nowDateStr() {
  const d = new Date();
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}${m}${day}`;
}

function generateInteractionId() {
  _counter += 1;
  const seq = String(_counter).padStart(3, "0");
  return `ix-${nowDateStr()}-${seq}`;
}

// ─── Default presentation ────────────────────────────────────────────────────

function defaultPresentation(overrides) {
  return Object.assign(
    {
      mode: "text_menu",
      color_profile: "auto",
      theme: "auto",
    },
    overrides || {},
  );
}

// ─── Builders ────────────────────────────────────────────────────────────────

/**
 * Build an approval_request interaction from approval pending state.
 *
 * opts:
 *   subject              {string}  short title shown in presenter
 *   detail               {string?} longer description
 *   risk_level           {string?} low|medium|high|critical
 *   originating_request_id {string} the runtime request that triggered this
 *   options              {Array}   [{id, label, recommended, shortcut?}]
 *   comment_mode         {string?} disabled|optional|required (default: disabled)
 *   presentation         {object?} override defaults
 */
function buildApprovalRequest(opts) {
  if (!opts || typeof opts.subject !== "string") {
    throw new Error("buildApprovalRequest: opts.subject is required");
  }
  if (!Array.isArray(opts.options) || opts.options.length < 2) {
    throw new Error("buildApprovalRequest: opts.options must have at least 2 entries");
  }
  return {
    schema: 1,
    interaction_id: generateInteractionId(),
    kind: "approval_request",
    status: "pending",
    title: opts.subject,
    message: opts.detail || opts.subject,
    selection_mode: "boolean",
    options: opts.options.map((o) => ({
      id: o.id,
      label: o.label,
      recommended: Boolean(o.recommended),
      shortcut: o.shortcut || null,
      description: o.description || null,
    })),
    comment_mode: opts.comment_mode || "disabled",
    presentation: defaultPresentation(opts.presentation),
    originating_request_id: opts.originating_request_id || "unknown",
    risk_level: opts.risk_level || null,
    created_at: new Date().toISOString(),
  };
}

/**
 * Build a restore_request interaction from restore_pending state.
 *
 * opts:
 *   restore_reason         {string}  reason code from runtime
 *   checkpoint_id          {string?} checkpoint to restore from
 *   originating_request_id {string}
 *   options                {Array?}  override default restore options
 *   presentation           {object?}
 */
function buildRestoreRequest(opts) {
  if (!opts || typeof opts.restore_reason !== "string") {
    throw new Error("buildRestoreRequest: opts.restore_reason is required");
  }
  const options = opts.options || [
    { id: "restore", label: "恢复检查点", recommended: true, shortcut: "r" },
    { id: "skip", label: "跳过恢复继续", recommended: false, shortcut: "s" },
    { id: "abort", label: "中止会话", recommended: false, shortcut: "a" },
  ];
  return {
    schema: 1,
    interaction_id: generateInteractionId(),
    kind: "restore_request",
    status: "pending",
    title: "需要恢复确认",
    message: `检测到恢复条件：${opts.restore_reason}${opts.checkpoint_id ? `（检查点：${opts.checkpoint_id}）` : ""}`,
    selection_mode: "single_select",
    options: options.map((o) => ({
      id: o.id,
      label: o.label,
      recommended: Boolean(o.recommended),
      shortcut: o.shortcut || null,
      description: o.description || null,
    })),
    comment_mode: opts.comment_mode || "optional",
    presentation: defaultPresentation(opts.presentation),
    originating_request_id: opts.originating_request_id || "unknown",
    restore_reason: opts.restore_reason,
    checkpoint_id: opts.checkpoint_id || null,
    created_at: new Date().toISOString(),
  };
}

/**
 * Build a question_request interaction from a question prompt.
 *
 * opts:
 *   question               {string}  the question text
 *   options                {Array}   [{id, label, recommended?}]
 *   originating_request_id {string}
 *   selection_mode         {string?} single_select|multi_select (default: single_select)
 *   comment_mode           {string?} disabled|optional|required (default: optional)
 *   presentation           {object?}
 */
function buildQuestionRequest(opts) {
  if (!opts || typeof opts.question !== "string") {
    throw new Error("buildQuestionRequest: opts.question is required");
  }
  const options = opts.options || [];
  return {
    schema: 1,
    interaction_id: generateInteractionId(),
    kind: "question_request",
    status: "pending",
    title: opts.title || "需要回答",
    message: opts.question,
    selection_mode: opts.selection_mode || "single_select",
    options: options.map((o) => ({
      id: o.id,
      label: o.label,
      recommended: Boolean(o.recommended),
      shortcut: o.shortcut || null,
      description: o.description || null,
    })),
    comment_mode: opts.comment_mode || "optional",
    presentation: defaultPresentation(opts.presentation),
    originating_request_id: opts.originating_request_id || "unknown",
    created_at: new Date().toISOString(),
  };
}

/**
 * Build an input_request interaction from an input prompt.
 *
 * opts:
 *   prompt                 {string}  what is needed from the user
 *   kind                   {string?} text|code|file_path|json|secret (default: text)
 *   language               {string?} hint for code kind
 *   example                {string?} example value
 *   originating_request_id {string}
 *   presentation           {object?}
 */
function buildInputRequest(opts) {
  if (!opts || typeof opts.prompt !== "string") {
    throw new Error("buildInputRequest: opts.prompt is required");
  }
  const inputKind = opts.kind || "text";
  // Map input kind to selection_mode
  const selectionMode = inputKind === "file_path" ? "path"
    : inputKind === "secret" ? "secret"
    : "text";
  return {
    schema: 1,
    interaction_id: generateInteractionId(),
    kind: "input_request",
    status: "pending",
    title: opts.title || "需要输入",
    message: opts.prompt,
    selection_mode: selectionMode,
    options: [],
    comment_mode: "disabled",
    presentation: defaultPresentation(opts.presentation),
    originating_request_id: opts.originating_request_id || "unknown",
    input_kind: inputKind,
    language: opts.language || null,
    example: opts.example || null,
    created_at: new Date().toISOString(),
  };
}

module.exports = {
  buildApprovalRequest,
  buildInputRequest,
  buildQuestionRequest,
  buildRestoreRequest,
};
