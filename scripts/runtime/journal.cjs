"use strict";

const crypto = require("node:crypto");
const {
  appendJournalEvents,
  readJournalEvents,
  readSession,
} = require("./store.cjs");

const ALLOWED_EVENTS = new Set([
  "session_started",
  "phase_entered",
  "phase_completed",
  "node_started",
  "write_recorded",
  "red_recorded",
  "green_recorded",
  "verification_recorded",
  "node_completed",
  "node_failed",
  "node_timed_out",
  "node_bypassed",
  "approval_requested",
  "approval_resolved",
  "approval_expired",
  "review_verdict_recorded",
  "checkpoint_created",
  "checkpoint_restored",
  "budget_exhausted",
  "session_stopped",
  "session_resumed",
  "validation_failed",
  "runtime_state_repaired",
]);

const ALLOWED_ACTORS = new Set([
  "runtime",
  "hook",
  "planner",
  "author",
  "spec_reviewer",
  "quality_reviewer",
  "reader",
  "auditor",
  "human",
  "adapter",
]);

function nowIso() {
  return new Date().toISOString();
}

function buildTraceId() {
  return crypto.randomBytes(6).toString("hex");
}

function normalizeEvent(rootDir, eventInput) {
  if (!eventInput || typeof eventInput !== "object") {
    throw new Error("event input must be an object");
  }
  const session = readSession(rootDir);
  const runId = eventInput.runId || session?.run_id;
  if (typeof runId !== "string" || runId.length === 0) {
    throw new Error("runId is required for journal events");
  }
  if (!ALLOWED_EVENTS.has(eventInput.event)) {
    throw new Error(`unsupported journal event: ${eventInput.event}`);
  }
  if (!ALLOWED_ACTORS.has(eventInput.actor)) {
    throw new Error(`unsupported event actor: ${eventInput.actor}`);
  }
  return {
    ts: eventInput.ts || nowIso(),
    run_id: runId,
    event: eventInput.event,
    phase: eventInput.phase || session?.phase?.current || "none",
    node_id: eventInput.nodeId || session?.node?.active_id || "none",
    actor: eventInput.actor,
    payload: eventInput.payload && typeof eventInput.payload === "object" ? eventInput.payload : {},
    trace_id: eventInput.traceId || buildTraceId(),
  };
}

function appendEvent(rootDir, eventInput) {
  const event = normalizeEvent(rootDir, eventInput);
  appendJournalEvents(rootDir, [event]);
  return event;
}

function appendEvents(rootDir, eventInputs) {
  const events = eventInputs.map((item) => normalizeEvent(rootDir, item));
  appendJournalEvents(rootDir, events);
  return events;
}

function readEvents(rootDir) {
  return readJournalEvents(rootDir);
}

function getJournalOffset(rootDir) {
  return readEvents(rootDir).length;
}

function findLatestEvent(rootDir, eventName) {
  const events = readEvents(rootDir);
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (events[index].event === eventName) {
      return events[index];
    }
  }
  return null;
}

module.exports = {
  ALLOWED_ACTORS,
  ALLOWED_EVENTS,
  appendEvent,
  appendEvents,
  findLatestEvent,
  getJournalOffset,
  normalizeEvent,
  readEvents,
};
