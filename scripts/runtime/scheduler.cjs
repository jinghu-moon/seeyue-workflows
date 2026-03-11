"use strict";

const { getActiveNode } = require("./runtime-state.cjs");

const TERMINAL_EVENTS = new Set(["node_completed", "node_failed", "node_timed_out", "node_bypassed"]);

function parseTimestamp(value) {
  const timestamp = Date.parse(String(value || ""));
  return Number.isFinite(timestamp) ? timestamp : null;
}

function clampNonNegative(value) {
  return Math.max(0, Number(value || 0));
}

function findLatestNodeStart(session, nodeId, journalEvents) {
  const runId = session?.run_id;
  const events = Array.isArray(journalEvents) ? journalEvents : [];
  let latestStart = null;

  for (const event of events) {
    if (!event || event.run_id !== runId || event.node_id !== nodeId) {
      continue;
    }
    if (event.event === "node_started") {
      latestStart = event;
    }
    if (latestStart && TERMINAL_EVENTS.has(event.event)) {
      latestStart = null;
    }
  }

  return latestStart;
}

function findFailureWindow(session, nodeId, journalEvents) {
  const runId = session?.run_id;
  const events = Array.isArray(journalEvents) ? journalEvents : [];
  const failures = events.filter((event) =>
    event
    && event.run_id === runId
    && event.node_id === nodeId
    && ["node_failed", "node_timed_out"].includes(event.event),
  );
  return {
    attempts_used: failures.length,
    latest_failure: failures.length > 0 ? failures[failures.length - 1] : null,
  };
}

function computeRetryDelaySeconds(retryPolicy, attemptsUsed) {
  if (!retryPolicy) {
    return 0;
  }
  const initialDelay = clampNonNegative(retryPolicy.initial_delay_seconds);
  if (initialDelay === 0) {
    return 0;
  }

  let delay = initialDelay;
  if (retryPolicy.backoff_mode === "exponential") {
    const exponent = Math.max(0, Number(attemptsUsed || 0) - 1);
    delay = initialDelay * (2 ** exponent);
  }
  const maxDelay = Number(retryPolicy.max_delay_seconds);
  if (Number.isFinite(maxDelay) && maxDelay >= 0) {
    delay = Math.min(delay, maxDelay);
  }
  return delay;
}

function deriveTimeoutSignal(session, taskGraph, journalEvents, now = new Date()) {
  const activeNode = getActiveNode(session, taskGraph);
  if (!activeNode || activeNode.status !== "in_progress" || !activeNode.timeout_policy) {
    return {
      tracked: false,
      triggered: false,
      node_id: activeNode?.id || null,
    };
  }

  const latestStart = findLatestNodeStart(session, activeNode.id, journalEvents);
  const startedAtMs = parseTimestamp(latestStart?.ts);
  if (startedAtMs === null) {
    return {
      tracked: false,
      triggered: false,
      node_id: activeNode.id,
      reason: "node_start_missing",
    };
  }

  const timeoutSeconds = Number(activeNode.timeout_policy.timeout_seconds || 0);
  const graceSeconds = clampNonNegative(activeNode.timeout_policy.grace_seconds);
  const deadlineMs = startedAtMs + ((timeoutSeconds + graceSeconds) * 1000);
  const nowMs = now instanceof Date ? now.getTime() : parseTimestamp(now);
  const remainingMs = deadlineMs - nowMs;

  return {
    tracked: true,
    triggered: remainingMs <= 0,
    node_id: activeNode.id,
    phase: activeNode.phase_id || session?.phase?.current || null,
    on_timeout: activeNode.timeout_policy.on_timeout,
    started_at: new Date(startedAtMs).toISOString(),
    deadline_at: new Date(deadlineMs).toISOString(),
    timeout_seconds: timeoutSeconds,
    grace_seconds: graceSeconds,
    remaining_ms: Math.max(0, remainingMs),
  };
}

function deriveRetryWindow(session, taskGraph, journalEvents, now = new Date()) {
  const activeNode = getActiveNode(session, taskGraph);
  if (!activeNode || activeNode.status !== "failed" || !activeNode.retry_policy) {
    return {
      tracked: false,
      ready: false,
      node_id: activeNode?.id || null,
    };
  }

  const failureWindow = findFailureWindow(session, activeNode.id, journalEvents);
  const latestFailureMs = parseTimestamp(failureWindow.latest_failure?.ts);
  if (!failureWindow.latest_failure || latestFailureMs === null) {
    return {
      tracked: false,
      ready: false,
      node_id: activeNode.id,
      attempts_used: failureWindow.attempts_used,
      reason: "failure_event_missing",
    };
  }

  const delaySeconds = computeRetryDelaySeconds(activeNode.retry_policy, failureWindow.attempts_used);
  const dueAtMs = latestFailureMs + (delaySeconds * 1000);
  const nowMs = now instanceof Date ? now.getTime() : parseTimestamp(now);
  const remainingMs = dueAtMs - nowMs;

  return {
    tracked: true,
    ready: remainingMs <= 0,
    node_id: activeNode.id,
    phase: activeNode.phase_id || session?.phase?.current || null,
    attempts_used: failureWindow.attempts_used,
    latest_failure_at: new Date(latestFailureMs).toISOString(),
    retry_due_at: new Date(dueAtMs).toISOString(),
    delay_seconds: delaySeconds,
    remaining_ms: Math.max(0, remainingMs),
    backoff_mode: activeNode.retry_policy.backoff_mode,
  };
}

function deriveRuntimeSignals(session, taskGraph, journalEvents, options = {}) {
  const now = options.now || new Date();
  return {
    timeout: deriveTimeoutSignal(session, taskGraph, journalEvents, now),
    retry: deriveRetryWindow(session, taskGraph, journalEvents, now),
  };
}

module.exports = {
  computeRetryDelaySeconds,
  deriveRetryWindow,
  deriveRuntimeSignals,
  deriveTimeoutSignal,
  findFailureWindow,
  findLatestNodeStart,
};
