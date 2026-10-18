import { create } from 'zustand';
import type { Project } from '../generated/Project';
import type { Task } from '../generated/Task';
import type { Settings } from '../generated/Settings';
import type { Bootstrap } from '../generated/Bootstrap';

interface Workspace {
  projects: Project[]; tasks: Task[]; settings: Settings; projectId: string | null; taskId: string | null; storage: string; browserAvailable: boolean;
  hydrate: (data: Bootstrap) => void; upsertTask: (task: Task) => void; upsertProject: (project: Project) => void;
  selectProject: (id: string) => void; selectTask: (id: string | null) => void; setSettings: (settings: Settings) => void; setBrowserAvailable: (value: boolean) => void;
}
export const useWorkspace = create<Workspace>((set) => ({
  projects: [], tasks: [], projectId: null, taskId: null, storage: '', browserAvailable: false,
  settings: { intelligence: { registryVersion: 0, automaticEscalation: true, fastContext: 16384, balancedContext: 32768, strongContext: 32768 }, theme: 'dark', model: null, modelProfiles: { fast: null, balanced: null, strong: null, strongEnabled: false }, contextSize: 8192, nodePath: null, pythonPath: null, gitPath: null, lastProjectId: null },
  hydrate: data => set(state => ({ projects: data.projects, tasks: data.tasks, settings: data.settings, storage: data.storagePath, browserAvailable: data.browserAvailable, projectId: state.projectId ?? data.settings.lastProjectId ?? data.projects[0]?.id ?? null })),
  upsertTask: task => set(state => {
    const previous = state.tasks.find(t => t.id === task.id);
    if (previous && previous.revision > task.revision) return state;
    return { tasks: [task, ...state.tasks.filter(t => t.id !== task.id)].sort((a, b) => b.createdAt.localeCompare(a.createdAt)) };
  }),
  upsertProject: project => set(state => ({ projects: [project, ...state.projects.filter(p => p.id !== project.id)], projectId: project.id, taskId: null })),
  selectProject: id => set({ projectId: id, taskId: null }), selectTask: id => set({ taskId: id }), setSettings: settings => set({ settings }),
  setBrowserAvailable: value => set({ browserAvailable: value }),
}));
