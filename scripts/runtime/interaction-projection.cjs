"use strict";

// interaction-projection.cjs — P1-N2: Runtime Schema Projection
//
// Reads and writes the session.yaml `interaction` block.
// This block reflects the current active interaction state within the run.
//
// interaction block structure (matches workflow/runtime.schema.yaml §interaction):
//   interaction:
//     active_interaction_id: ix-20260318-001   # canonical field
//     pending_count: 1
//     last_dispatched_at: null
//     blocking_kind: approval                  # optional runtime context
//     blocking_reason: destructive_write_requires_approval  # optional

const { readSession, writeSession } = require("./store.cjs");

/**
 * Extract the interaction block from a session object.
 * Returns null if the session has no interaction block or no active interaction.
 *
 * @param {object} session — in-memory session object
 * @returns {object|null}
 */
function getInteractionBlock(session) {
  if (!session || !session.interaction) {
    return null;
  }
  // Treat block as absent if active_interaction_id is null/undefined
  if (!session.interaction.active_interaction_id) {
    return null;
  }
  return session.interaction;
}

/**
 * Write an updated session.yaml with the given interaction block merged in.
 * Does NOT mutate the in-memory session argument.
 *
 * @param {string} rootDir
 * @param {object} session — current session (used as base)
 * @param {object} interactionBlock — new interaction block to set
 */
function setInteractionBlock(rootDir, session, interactionBlock) {
  if (!interactionBlock || typeof interactionBlock.active_interaction_id !== "string") {
    throw new Error("interactionBlock.active_interaction_id is required");
  }
  const updated = Object.assign({}, session, {
    interaction: Object.assign({}, interactionBlock),
  });
  writeSession(rootDir, updated);
}

/**
 * Remove the active interaction from session.yaml by resetting to defaults.
 * Reads the latest session from disk, clears active_interaction_id, re-writes.
 *
 * @param {string} rootDir
 * @param {object} session — current session (used as base)
 */
function clearInteractionBlock(rootDir, session) {
  const updated = Object.assign({}, session, {
    interaction: {
      active_interaction_id: null,
      pending_count: 0,
      last_dispatched_at: null,
    },
  });
  writeSession(rootDir, updated);
}

module.exports = {
  clearInteractionBlock,
  getInteractionBlock,
  setInteractionBlock,
};
