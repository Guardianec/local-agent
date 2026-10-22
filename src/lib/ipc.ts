import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Bootstrap } from '../generated/Bootstrap';
import type { Task } from '../generated/Task';
import type { Project } from '../generated/Project';
import type { Review } from '../generated/Review';
import type { RuntimeInfo } from '../generated/RuntimeInfo';
import type { ModelInfo } from '../generated/ModelInfo';
import type { Settings } from '../generated/Settings';
import type { Mode } from '../generated/Mode';
import type { HandoffPreview } from '../generated/HandoffPreview';
import type { AppError } from '../generated/AppError';
import type { ModelCalibration } from '../generated/ModelCalibration';
import type { ContentBlock } from '../generated/ContentBlock';
import type { ImageAttachment } from '../generated/ImageAttachment';
import type { ModelSpec } from '../generated/ModelSpec';
import type { ModelInstallProgress } from '../generated/ModelInstallProgress';
import type { CleanupCandidate } from '../generated/CleanupCandidate';
import type { Integration } from '../generated/Integration';

async function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw { code: 'DESKTOP_REQUIRED', message: 'Open the Local Agent desktop app to work with project files.', details: 'The browser preview has no filesystem or command access.' } satisfies AppError;
  return invoke<T>(name, args);
}
export function errorOf(error: unknown): AppError {
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const e = error as Partial<AppError>;
    return { code: e.code ?? 'UNEXPECTED', message: e.message ?? 'Something went wrong.', details: e.details ?? null };
  }
  return { code: 'UNEXPECTED', message: 'The operation could not be completed.', details: String(error) };
}
export const api = {
  integrationStatus: () => call<Record<string, { state: string; authentication: string; lastError: string | null; protocol: string; tools: [string, string][]; withheld: number; checkedAt: string }>>('integration_status'),
  integrations: () => call<Integration[]>('integrations'),
  saveIntegration: (config: Integration, secret: string | null, confirmed: boolean) => call<Integration>('save_integration', { config, secret, confirmed }),
  removeIntegration: (id: string) => call<void>('remove_integration', { id }),
  testIntegration: (id: string) => call<{ status: string; protocol: string; capability: string; tools: { name: string; effect: string }[] }>('test_integration', { id }),
  bootstrap: () => call<Bootstrap>('bootstrap'),
  openProject: () => call<Project | null>('open_project'),
  browserWorkspace: () => call<Project>('browser_workspace'),
  startTask: (projectId: string, request: string, preset: string, model: string | null, mode: Mode, parentTaskId: string | null, inputs: ContentBlock[] = []) => call<Task>('start_task', { projectId, request, preset, model, mode, parentTaskId, inputs }),
  task: (taskId: string) => call<Task>('get_task', { taskId }),
  cancel: (taskId: string) => call<void>('cancel_task', { taskId }),
  answer: (taskId: string, approvalId: string, choice: string) => call<void>('answer_approval', { taskId, approvalId, choice }),
  review: (taskId: string) => call<Review>('get_review', { taskId }),
  changeFiles: (taskId: string, revision: number, operation: string) => call<Task>('change_files', { taskId, revision, operation }),
  runtime: () => call<RuntimeInfo>('runtime_status'),
  probe: (name: string) => call<ModelInfo>('probe_model', { name }),
  calibrate: (name: string) => call<ModelCalibration>('calibrate_model', { name }),
  settings: (settings: Settings) => call<Settings>('save_settings', { settings }),
  chooseTool: (kind: string) => call<Settings>('choose_tool', { kind }),
  handoff: (taskId: string) => call<HandoffPreview>('handoff_preview', { taskId }),
  exportHandoff: (previewId: string) => call<string | null>('export_handoff', { previewId }),
  help: (action: string) => call<void>('open_runtime_help', { action }),
  installBrowser: () => call<void>('install_browser'),
  catalog: () => call<ModelSpec[]>('model_catalog'),
  installModel: (tier: string) => call<void>('install_model', { tier }),
  installProgress: () => call<ModelInstallProgress | null>('model_install_progress'),
  cleanupCandidates: () => call<CleanupCandidate[]>('model_cleanup_candidates'),
  removeModel: (name: string) => call<void>('remove_obsolete_model', { name }),
  chooseImages: () => call<ImageAttachment[]>('choose_images'),
  ingestImage: (name: string, bytesBase64: string, source: string) => call<ImageAttachment>('ingest_image', { name, bytesBase64, source }),
  imagePreview: (attachmentId: string) => call<string>('image_preview', { attachmentId }),
  onTask: async (handler: (task: Task) => void) => isTauri() ? listen<Task>('task-updated', e => handler(e.payload)) : () => {},
};
export const activeTask = (task: Task | undefined | null) => !!task && ['queued', 'preparing', 'routing', 'running', 'waiting_approval', 'validating', 'stopping'].includes(task.state);
