"use strict";

// interaction-router.cjs — P1-N6: Controller/Router Integration
//
// Blocker-first interaction handling.
// These functions are used by controller.cjs before normal phase advance
// to check whether an active interaction must be resolved first.

const { getInteractionBlock } = require("./interaction-projection.cjs");

/**
 * Returns true if the session has an active pending interaction that
 // must be resolved before normal phase advance can continue.
 *
 * @param {object} session — current session object
 * @returns {boolean}
 */
function shouldBlockForInteraction(session) {
  const block = getInteractionBlock(session);
  return block !== null && typeof block.active_interaction_id === "string" && block.active_interaction_id.length > 0;
}

/**
 * Returns a blocker descriptor for the current active interaction,
 * suitable for use in recommended_next or block_reason.
 * Returns null if no active interaction.
 *
 * Blocker descriptor shape:
 *   { type: "interaction_pending", interaction_id, blocking_kind, reason, priority }
 *
 * @param {object} session — current session object
 * @returns {object|null}
 */
function getInteractionBlocker(session) {
  const block = getInteractionBlock(session);
  if (!block) {
    return null;
  }
  return {
    type: "interaction_pending",
    interaction_id: block.active_interaction_id,
    blocking_kind: block.blocking_kind || "unknown",
    reason: block.blocking_reason || "active_interaction_requires_resolution",
    pending_count: block.pending_count || 1,
    priority: "now",
  };
}

module.exports = {
  getInteractionBlocker,
  shouldBlockForInteraction,
};
