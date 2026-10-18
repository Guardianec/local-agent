import type { Task } from '../../generated/Task';
import { activeTask } from '../../lib/ipc';
import registry from '../../../assets/model-registry.json';
import pkg from '../../../package.json';

export function outcomeText(task: Task): string {
  if (activeTask(task)) return '';
  return task.result?.userMessage?.trim() || task.summary.trim() || 'Something ended unexpectedly before I could produce a response. Retry or open technical details.';
}

// Allowlisted metadata only. Do not copy event prose, paths, custom model names or errors.
export function diagnostics(task: Task): string {
  const knownStates = new Set(['queued', 'preparing', 'routing', 'running', 'waiting_approval', 'validating', 'stopping', 'review_ready', 'failed', 'cancelled', 'escalation_ready', 'interrupted']);
  const failures = new Set(['MODEL_UNAVAILABLE', 'MODEL_LOAD_FAILED', 'MODEL_REQUEST_FAILED', 'MODEL_RESPONSE_EMPTY', 'MODEL_OUTPUT_INVALID', 'STREAM_INTERRUPTED', 'TOOL_CALL_INVALID', 'TOOL_EXECUTION_FAILED', 'TOOL_UNAVAILABLE', 'TOOL_PERMISSION_DENIED', 'COMMAND_TIMEOUT', 'VALIDATION_FAILED', 'ITERATION_LIMIT', 'CONTEXT_LIMIT', 'MEMORY_PRESSURE', 'MODEL_STAGNATION', 'USER_CANCELLED', 'ESCALATION_FAILED', 'CODEX_HANDOFF_FAILED', 'UNKNOWN_INTERNAL_ERROR']);
  const state = (s: string) => knownStates.has(s) ? s : 'unknown';
  const transitions = task.events.filter(e => e.kind === 'state').map(e => e.detail.split(' → ')).filter(parts => parts.length === 2 && parts.every(s => knownStates.has(s))).map(parts => parts.join(' → '));
  return JSON.stringify({ version: pkg.version, os: 'Windows', taskId: /^[a-zA-Z0-9-]{1,64}$/.test(task.id) ? task.id : 'omitted', intent: ['converse', 'inspect', 'execute'].includes(task.intent) ? task.intent : 'legacy', model: registry.tiers.some(t => t.modelId === task.model) ? task.model : 'custom or unavailable', status: state(task.state), failure: failures.has(task.result?.failure?.category ?? '') ? task.result?.failure?.category : null, retries: task.metrics?.retries ?? 0, toolCalls: task.toolCalls, transitions }, null, 2);
}
