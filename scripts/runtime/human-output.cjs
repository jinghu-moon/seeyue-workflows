"use strict";

const APPROVAL_MODE_LABELS = {
  manual_required: "人工审批",
  never_auto: "必须人工审批",
};

const GRANT_SCOPE_LABELS = {
  once: "单次授权",
  session: "会话授权",
};

const RISK_CLASS_LABELS = {
  low: "低",
  medium: "中",
  high: "高",
  critical: "关键",
};

const RESTORE_REASON_LABELS = {
  restore_pending: "存在待处理恢复",
  missing_terminal_event: "检测到中断，需要从检查点恢复",
  pending_approval_survived_restart: "存在未完成审批，不能自动继续",
  missing_tool_call_metadata: "缺少工具调用元数据，无法自动恢复",
  target_snapshot_metadata_missing: "恢复元数据缺失，无法自动恢复",
  target_snapshot_content_missing: "恢复内容快照缺失，无法自动恢复",
  target_snapshot_unsupported_kind: "目标类型不支持自动恢复",
  target_snapshot_requires_manual_restore: "目标仅保留元数据，需人工恢复",
  capsule_snapshot_missing: "Capsule 快照缺失，需人工确认上下文",
  capsule_snapshot_invalid: "Capsule 快照无效，需人工确认上下文",
};

function toArray(value) {
  return Array.isArray(value) ? value : (value === undefined || value === null ? [] : [value]);
}

function formatLabeledToken(value, labels) {
  const token = String(value || "unknown").trim() || "unknown";
  const label = labels[token];
  return label ? `${label} (${token})` : token;
}

function formatGrantScopes(grantScopes) {
  const values = toArray(grantScopes);
  if (values.length === 0) {
    return formatLabeledToken("once", GRANT_SCOPE_LABELS);
  }
  return values.map((value) => formatLabeledToken(value, GRANT_SCOPE_LABELS)).join(" / ");
}

function formatRecommendedNext(items) {
  const nextItems = toArray(items);
  if (nextItems.length === 0) {
    return "无";
  }
  return nextItems
    .map((item) => {
      const type = String(item?.type || "unknown").trim() || "unknown";
      const target = String(item?.target || "unknown").trim() || "unknown";
      return `${type}:${target}`;
    })
    .join("，");
}

function buildShortApprovalRequest(options = {}) {
  return [
    "需要人工审批",
    `操作类型：${String(options.actionLabel || options.action || "未知操作").trim() || "未知操作"}`,
    `影响范围：${String(options.targetRef || options.target || "unknown").trim() || "unknown"}`,
    `风险等级：${formatLabeledToken(options.riskClass || options.risk_class || "unknown", RISK_CLASS_LABELS)}`,
    `审批模式：${formatLabeledToken(options.approvalMode || options.approval_mode || "manual_required", APPROVAL_MODE_LABELS)}`,
    `授权范围：${formatGrantScopes(options.grantScopes || options.grant_scopes)}`,
    "请确认是否继续？【回复：确认 / 继续 / 批准】",
  ];
}

function buildShortRestoreRequest(options = {}) {
  const reason = String(options.reason || options.restore_reason || "restore_pending").trim() || "restore_pending";
  const checkpointId = String(options.checkpointId || options.checkpoint_id || "none").trim() || "none";
  const activeNode = String(options.activeNode || options.active_node || "none").trim() || "none";
  const targetRef = String(options.targetRef || options.target_ref || "unknown").trim() || "unknown";
  const recommendedNext = formatRecommendedNext(options.recommendedNext || options.recommended_next);

  return [
    "需要人工处理恢复",
    `恢复原因：${formatLabeledToken(reason, RESTORE_REASON_LABELS)}`,
    `检查点：${checkpointId}`,
    `当前节点：${activeNode}`,
    `影响范围：${targetRef}`,
    `建议下一步：${recommendedNext}`,
    "请先处理恢复问题，再继续自动执行。",
  ];
}

module.exports = {
  APPROVAL_MODE_LABELS,
  GRANT_SCOPE_LABELS,
  RESTORE_REASON_LABELS,
  RISK_CLASS_LABELS,
  buildShortApprovalRequest,
  buildShortRestoreRequest,
  formatRecommendedNext,
};

