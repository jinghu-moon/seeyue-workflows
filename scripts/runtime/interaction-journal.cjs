"use strict";

// interaction-journal.cjs — P1-N7: Journal/Checkpoint Interaction Events
//
// Adds interaction-specific event types to the journal.
// Wraps journal.cjs appendEvent with validated interaction event support.

const { appendJournalEvents, readSession } = require("./store.cjs");
const crypto = require("node:crypto");

// ─── Interaction event type constants ────────────────────────────────────────

const INTERACTION_EVENTS = {
  interaction_created:   "interaction_created",
  interaction_presented: "interaction_presented",
  interaction_answered:  "interaction_answered",
  interaction_cancelled: "interaction_cancelled",
  interaction_expired:   "interaction_expired",
};

const INTERACTION_EVENT_SET = new Set(Object.values(INTERACTION_EVENTS));

// ─── Helpers ─────────────────────────────────────────────────────────────────

function nowIso() {
  return new Date().toISOString();
}

function buildTraceId() {
  return crypto.randomBytes(6).toString("hex");
}

// ─── Public API ──────────────────────────────────────────────────────────────

/**
 * Append an interaction lifecycle event to journal.jsonl.
 *
 * @param {string} rootDir
 * @param {object} eventInput
 *   eventInput.event           {string}  one of INTERACTION_EVENTS values
 *   eventInput.runId           {string?} defaults to session.run_id
 *   eventInput.interactionId   {string}  the ix-* id
 *   eventInput.blockingKind    {string?} approval|restore|question|input
 *   eventInput.payload         {object?} extra payload fields
 */
function appendInteractionEvent(rootDir, eventInput) {
  if (!eventInput || !INTERACTION_EVENT_SET.has(eventInput.event)) {
    throw new Error(
      `appendInteractionEvent: unsupported event "${eventInput?.event}". ` +
      `Must be one of: ${[...INTERACTION_EVENT_SET].join(", ")}`
    );
  }
  if (!eventInput.interactionId || typeof eventInput.interactionId !== "string") {
    throw new Error("appendInteractionEvent: interactionId is required");
  }

  const session = readSession(rootDir);
  const runId = eventInput.runId || session?.run_id;
  if (!runId) {
    throw new Error("appendInteractionEvent: runId is required (provide explicitly or via session.yaml)");
  }

  const record = {
    ts:         eventInput.ts || nowIso(),
    run_id:     runId,
    event:      eventInput.event,
    phase:      eventInput.phase || session?.phase?.current || "none",
    node_id:    eventInput.nodeId || session?.node?.active_id || "none",
    actor:      eventInput.actor || "runtime",
    trace_id:   eventInput.traceId || buildTraceId(),
    payload:    Object.assign(
      {
        interaction_id: eventInput.interactionId,
        blocking_kind:  eventInput.blockingKind || null,
      },
      eventInput.payload || {},
    ),
  };

  appendJournalEvents(rootDir, [record]);
  return record;
}

module.exports = {
  INTERACTION_EVENTS,
  appendInteractionEvent,
};
