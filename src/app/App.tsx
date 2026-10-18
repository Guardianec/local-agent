import modelRegistry from '../../assets/model-registry.json';
import { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowRight, ArrowUp, BookOpen, Check, ChevronDown, ChevronRight, CircleDot, Code2, Cpu, FileCode2, Folder, FolderOpen, GitBranch, HelpCircle, Layers3, LoaderCircle, PanelLeftClose, Play, Plus, Search, Settings2, ShieldCheck, Square, TerminalSquare, TriangleAlert, X } from 'lucide-react';
import { api, activeTask, errorOf } from '../lib/ipc';
import { useWorkspace } from '../state/workspace';
import type { AppError } from '../generated/AppError';
import type { Mode } from '../generated/Mode';
import type { Review } from '../generated/Review';
import type { RuntimeInfo } from '../generated/RuntimeInfo';
import type { HandoffPreview } from '../generated/HandoffPreview';
import { ImageInputs, ImagePreview, ingestFiles } from '../features/tasks/ImageInputs';
import type { ImageAttachment } from '../generated/ImageAttachment';
import { Activity, Checks } from '../features/tasks/Activity';
import { ReviewPanel } from '../features/review/ReviewPanel';
import { ModelsDialog } from '../features/models/ModelsDialog';
import { ApprovalDialog } from '../features/permissions/ApprovalDialog';
import { HandoffDialog } from '../features/handoffs/HandoffDialog';
import { SettingsDialog } from '../features/settings/SettingsDialog';
import { Dialog } from '../components/ui/Dialog';
import { BrowserDialog } from '../features/browser/BrowserDialog';

const modeLabels: Record<Mode, string> = { observe: 'Observe', assisted: 'Ask before edits', project: 'Project permissions' };
const stateLabel: Record<string, string> = { queued: 'Queued', preparing: 'Preparing task copy', routing: 'Choosing a strategy', running: 'Working locally', waiting_approval: 'Permission required', validating: 'Checking changes', stopping: 'Stopping', review_ready: 'Ready for review', escalation_ready: 'Handoff recommended', failed: 'Needs attention', cancelled: 'Stopped', interrupted: 'Interrupted' };
const quickActions = [
  { id: 'tests', icon: Play, title: 'Run tests', text: 'Run the project tests.', description: 'Check the current project' },
  { id: 'fix_lint', icon: Code2, title: 'Fix lint errors', text: 'Fix lint errors using the project linter.', description: 'Use the existing lint recipe' },
  { id: 'search', icon: Search, title: 'Search project', text: '', description: 'Find code without a model' },
  { id: 'explain', icon: BookOpen, title: 'Explain a file', text: '', description: 'Understand a focused piece of code' },
];

export default function App() {
  const workspace = useWorkspace();
  const { projects, tasks, projectId, taskId, settings } = workspace;
  const project = projects.find(p => p.id === projectId);
  const task = tasks.find(t => t.id === taskId);
  const running = tasks.find(activeTask);
  const [loading, setLoading] = useState(true), [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null), [notice, setNotice] = useState('');
  const [runtime, setRuntime] = useState<RuntimeInfo | null>(null), [runtimeLoading, setRuntimeLoading] = useState(false);
  const [modelsOpen, setModelsOpen] = useState(false), [settingsOpen, setSettingsOpen] = useState(false);
  const [calibrating, setCalibrating] = useState<string | null>(null);
  const [modelOverride, setModelOverride] = useState('auto');
  const [images, setImages] = useState<ImageAttachment[]>([]);
  const [ingesting, setIngesting] = useState(false);
  const [probing, setProbing] = useState<string | null>(null), [approvalOpen, setApprovalOpen] = useState(true);
  const [handoff, setHandoff] = useState<HandoffPreview | null>(null);
  const [request, setRequest] = useState(''), [preset, setPreset] = useState('task'), [file, setFile] = useState('');
  const [mode, setMode] = useState<Mode>('assisted'), [tab, setTab] = useState('activity');
  const [review, setReview] = useState<Review | null>(null), [confirm, setConfirm] = useState<string | null>(null);
  const [sidebar, setSidebar] = useState(true);
  const [browserOpen, setBrowserOpen] = useState(false), [installingBrowser, setInstallingBrowser] = useState(false);
  const input = useRef<HTMLTextAreaElement>(null);
  const draftEpoch = useRef(0);
  const draftToken = draftEpoch.current;
  const onError = useCallback((e: unknown) => { setError(errorOf(e)); }, []);
  const refreshRuntime = useCallback(async () => { setRuntimeLoading(true); try { setRuntime(await api.runtime()); } catch (e) { if (errorOf(e).code !== 'DESKTOP_REQUIRED') onError(e); } finally { setRuntimeLoading(false); } }, [onError]);
  const openProject = useCallback(async () => { setBusy(true); setError(null); try { const result = await api.openProject(); if (result) { useWorkspace.getState().upsertProject(result); setNotice(`Opened ${result.name}`); } } catch (e) { onError(e); } finally { setBusy(false); } }, [onError]);
  useEffect(() => {
    let disposed = false; let unlisten: (() => void) | undefined;
    void (async () => {
      try {
        unlisten = await api.onTask(value => { if (!disposed) useWorkspace.getState().upsertTask(value); });
        const data = await api.bootstrap(); if (!disposed) useWorkspace.getState().hydrate(data);
      } catch (e) { if (!disposed) onError(e); } finally { if (!disposed) setLoading(false); }
      if (!disposed) void refreshRuntime();
    })();
    return () => { disposed = true; unlisten?.(); };
  }, [onError, refreshRuntime]);
  useEffect(() => { const media = matchMedia('(prefers-color-scheme: light)'); const update = () => { document.documentElement.dataset.theme = settings.theme === 'system' ? (media.matches ? 'light' : 'dark') : settings.theme; }; update(); media.addEventListener('change', update); return () => media.removeEventListener('change', update); }, [settings.theme]);
  useEffect(() => { setMode(project?.mode ?? 'assisted'); setRequest(''); setFile(''); setPreset('task'); setImages([]); draftEpoch.current += 1; }, [project?.id, project?.mode]);
  useEffect(() => { if (!taskId) setModelOverride('auto'); }, [taskId]);
  useEffect(() => { if (task?.approval) setApprovalOpen(true); }, [task?.approval?.id]);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => { void api.task(running.id).then(workspace.upsertTask).catch(onError); }, 1500);
    return () => clearInterval(timer);
  }, [running?.id, workspace.upsertTask, onError]);
  useEffect(() => {
    const timer = setInterval(() => { if (!document.hidden && (running || modelsOpen)) void refreshRuntime(); }, running ? 5000 : 30000);
    return () => clearInterval(timer);
  }, [running?.id, modelsOpen, refreshRuntime]);
  useEffect(() => {
    let ignore = false; setReview(null);
    if (task && !activeTask(task)) void api.review(task.id).then(result => { if (!ignore) setReview(result); }).catch(onError);
    return () => { ignore = true; };
  }, [task?.id, task?.revision, onError]);
  useEffect(() => { if (task && !activeTask(task) && task.changeState === 'proposed') setTab('changes'); }, [task?.id, task?.state, task?.changeState]);
  useEffect(() => { const key = (e: KeyboardEvent) => { if (e.ctrlKey && e.key.toLowerCase() === 'o') { e.preventDefault(); void openProject(); } if (e.ctrlKey && e.shiftKey && e.key === 'Escape' && running) { e.preventDefault(); void api.cancel(running.id).catch(onError); } }; window.addEventListener('keydown', key); return () => window.removeEventListener('keydown', key); }, [openProject, running?.id, onError]);

  async function start(parentTaskId: string | null = null) {
    if (!project || busy || running || ingesting) return;
    const text = preset === 'explain' && file ? `Explain ${file}. ${request}` : request;
    if (!text.trim()) { input.current?.focus(); return; }
    setBusy(true); setError(null);
    try { const created = await api.startTask(project.id, text, preset, modelOverride === 'auto' ? null : modelOverride, mode, parentTaskId, images.map(image => ({ type: 'image', attachment_id: image.id }))); workspace.upsertTask(created); workspace.selectTask(created.id); setRequest(''); setImages([]); setTab('activity'); }
    catch (e) { onError(e); } finally { setBusy(false); }
  }
  async function addImages(files: File[], source: 'drop' | 'clipboard') {
    if (ingesting || running || busy) return;
    if (files.length + images.length > 4) { onError({ code: 'IMAGE_COUNT', message: 'Attach up to four images.' }); return; }
    const epoch = draftEpoch.current; const targetProject = projectId;
    setIngesting(true);
    try { const added = await ingestFiles(files, source); if (draftEpoch.current === epoch && useWorkspace.getState().projectId === targetProject) setImages(current => [...current, ...added]); } catch (e) { onError(e); } finally { setIngesting(false); }
  }
  async function saveSettings(next: typeof settings) { try { workspace.setSettings(await api.settings(next)); } catch (e) { onError(e); } }
  async function prepareHandoff() {
    if (!project) return;
    setBusy(true); setError(null);
    try {
      let target = task;
      if (!target) { target = await api.startTask(project.id, request || 'Prepare context for a coding agent to inspect this project.', 'handoff', null, mode, null); workspace.upsertTask(target); workspace.selectTask(target.id); }
      setHandoff(await api.handoff(target.id));
    } catch (e) { onError(e); } finally { setBusy(false); }
  }
  async function performOperation() {
    if (!task || !confirm) return;
    const operation = confirm; setConfirm(null); setBusy(true); setError(null);
    try { const result = await api.changeFiles(task.id, task.revision, operation); workspace.upsertTask(result); setNotice(operation === 'apply' ? 'Changes applied to your project' : operation === 'undo' ? 'Applied changes undone' : operation === 'discard' ? 'Proposal discarded' : 'Project files restored'); }
    catch (e) { onError(e); void api.task(task.id).then(workspace.upsertTask); } finally { setBusy(false); }
  }
  const taskList = tasks.filter(t => t.projectId === projectId);
  const selectedModel = runtime?.models.find(m => m.name === (settings.modelProfiles.balanced ?? settings.modelProfiles.fast));
  const resolvedTheme = document.documentElement.dataset.theme ?? 'dark';
  const status = task ? stateLabel[task.state] ?? task.state : 'Ready';
  const selectedPreset = quickActions.find(a => a.id === preset);

  return <div className={`app-shell ${sidebar ? '' : 'rail-collapsed'}`}>
    <aside className="sidebar"><div className="brand"><span className="brand-mark"><Code2 size={22}/></span><strong>Local Agent</strong><span className="version">0.3</span></div>
      <div className="sidebar-section"><div className="section-label">PROJECTS<button className="icon-button" aria-label="Open project" disabled={busy} onClick={() => void openProject()}><Plus size={15}/></button></div>
        {projects.map(p => <button className={`project-item ${p.id === projectId ? 'selected' : ''}`} key={p.id} title={p.path} onClick={() => workspace.selectProject(p.id)}><Folder size={16}/><span>{p.name}</span>{p.id === projectId && <ChevronDown size={13}/>}</button>)}
        <button className="project-item add-project" disabled={busy} onClick={() => void openProject()}><Plus size={16}/><span>Open project</span></button>
      </div>
      <div className="task-navigation"><div className="section-label">TASKS<button className="icon-button" aria-label="New task" disabled={!project || !!running} onClick={() => { draftEpoch.current += 1; workspace.selectTask(null); setRequest(''); setImages([]); setModelOverride('auto'); setPreset('task'); setTab('activity'); setTimeout(() => input.current?.focus(), 0); }}><Plus size={15}/></button></div>
        {taskList.length ? taskList.map(t => <button className={`task-item ${taskId === t.id ? 'selected' : ''}`} onClick={() => { workspace.selectTask(t.id); setTab('activity'); }} title={t.request} key={t.id}><span className={`task-indicator ${t.state}`}>{activeTask(t) ? <LoaderCircle size={13} className="spin"/> : t.state === 'failed' || t.state === 'escalation_ready' ? <CircleDot size={13}/> : <Check size={13}/>}</span><span>{t.request}</span></button>) : <p className="sidebar-empty">Your tasks will appear here.</p>}
      </div>
      <div className="sidebar-footer"><button className="runtime-sidebar" onClick={() => setModelsOpen(true)}><span className={`status-dot ${runtime?.status === 'ready' ? 'online' : ''}`}/><div><b>{runtime?.status === 'ready' ? 'Ollama connected' : 'Connect a local model'}</b><span>{runtime?.status === 'ready' ? 'Running on this PC' : 'Set up Ollama to get started'}</span></div><ChevronRight size={14}/></button><button className="sidebar-settings" onClick={() => setSettingsOpen(true)}><Settings2 size={16}/><span>Settings</span></button><div className="local-first"><ShieldCheck size={12}/>Local by default</div></div>
    </aside>
    <main className="workspace"><header className="topbar"><div className="breadcrumbs"><button className="icon-button rail-toggle" aria-label={sidebar ? 'Collapse sidebar' : 'Expand sidebar'} onClick={() => setSidebar(!sidebar)}><PanelLeftClose size={17}/></button><Folder size={16}/><span title={project?.path}>{project?.name ?? 'Workspace'}</span>{task && <><ChevronRight size={13}/><span className="breadcrumb-task">{task.request}</span></>}</div><div className="header-actions"><button className="model-button" onClick={() => setModelsOpen(true)}><Cpu size={15}/><span>{(settings.modelProfiles.balanced || settings.modelProfiles.fast) ? 'Auto routing' : 'Choose model'}</span><ChevronDown size={12}/></button><button className="icon-button" aria-label="Settings" onClick={() => setSettingsOpen(true)}><Settings2 size={17}/></button></div></header>
      {error && <div className={`error-banner ${error.code === 'DESKTOP_REQUIRED' ? 'info' : ''}`} role="alert"><TriangleAlert size={17}/><div><b>{error.message}</b>{error.details && <details><summary>View technical details</summary><pre>{error.details}</pre></details>}</div><button className="icon-button" aria-label="Dismiss error" onClick={() => setError(null)}><X size={15}/></button></div>}
      {notice && <div className="notice" role="status"><Check size={15}/>{notice}<button className="icon-button" aria-label="Dismiss notice" onClick={() => setNotice('')}><X size={14}/></button></div>}
      {loading ? <div className="pane-empty"><LoaderCircle className="spin"/><h3>Opening your workspace…</h3></div> : !project ? <div className="welcome"><div className="welcome-emblem"><Code2 size={40}/></div><span className="eyebrow">YOUR MACHINE. YOUR WORKSPACE.</span><h1>A little less busywork.</h1><p>Give routine coding tasks a place to run.<br/>Work locally, review the changes, keep what helps.</p><button className="button primary large" disabled={busy} onClick={() => void openProject()}><FolderOpen size={17}/>Open a project<ArrowRight size={16}/></button><button className="text-button setup-link" onClick={() => setModelsOpen(true)}>Set up a local model<ChevronRight size={14}/></button><div className="welcome-details"><span><Layers3 size={15}/>Focused tasks</span><span><FileCode2 size={15}/>Review before Apply</span><span><ShieldCheck size={15}/>Recoverable changes</span></div></div> : <>
        {!task ? <div className="project-home"><div className="home-heading"><span className="eyebrow">PROJECT WORKSPACE</span><h1>What needs doing?</h1><p>Small tasks, clear changes. Everything runs on your PC.</p></div>
          <div className="composer home-composer" onDragOver={e => { if (e.dataTransfer.types.includes('Files')) e.preventDefault(); }} onDrop={e => { e.preventDefault(); void addImages(Array.from(e.dataTransfer.files), 'drop'); }}><div className="composer-top"><span><Code2 size={15}/>{selectedPreset?.title ?? 'New coding task'}</span><select aria-label="Permission mode" value={mode} onChange={e => setMode(e.target.value as Mode)}>{Object.entries(modeLabels).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</select></div>{preset === 'explain' && <select className="file-select" aria-label="File to explain" value={file} onChange={e => setFile(e.target.value)}><option value="">Choose a project file…</option>{project.files.filter(p => /\.(tsx?|jsx?|cjs|mjs|py|rs|md|css|html|json)$/.test(p)).slice(0, 1000).map(path => <option key={path}>{path}</option>)}</select>}
            <textarea onPaste={e => { const files = Array.from(e.clipboardData.files); if (files.length) { e.preventDefault(); void addImages(files, 'clipboard'); } }} ref={input} aria-label="Task description" value={request} onChange={e => setRequest(e.target.value)} onKeyDown={e => { if (e.ctrlKey && e.key === 'Enter') { e.preventDefault(); void start(); } }} placeholder={preset === 'search' ? 'Search for a function, phrase, or error message…' : preset === 'explain' ? 'What would you like to understand about this file?' : 'Fix the lint errors and run the tests…'} rows={4}/>
            <ImageInputs images={images} busy={busy || !!running || ingesting} onChange={next => { if (draftEpoch.current === draftToken && useWorkspace.getState().projectId === projectId) setImages(next); }} onError={onError}/><div className="task-model-choice"><label htmlFor="task-model">Model</label><select id="task-model" aria-label="Task model" value={modelOverride} onChange={e => setModelOverride(e.target.value)}><option value="auto">Auto · choose and escalate when needed</option>{([['fast', 'Fast'], ['balanced', 'Balanced'], ['strong', 'Strong']] as const).map(([key, label]) => settings.modelProfiles[key] && <option key={key} value={'@profile/' + key} disabled={key === 'strong' && !settings.modelProfiles.strongEnabled}>{label} · {modelRegistry.tiers.find(spec => spec.tier === key && spec.modelId === settings.modelProfiles[key])?.displayName ?? settings.modelProfiles[key]}{key === 'strong' ? ' · uses RAM' : ''}</option>)}</select></div><div className="composer-bottom"><span>{preset === 'search' || preset === 'tests' ? <><TerminalSquare size={13}/>Deterministic tools first</> : <><span className="status-dot"/>Changes start in a private copy</>}</span><button className="button primary start-button" disabled={busy || !!running || ingesting || (!request.trim() && !(preset === 'explain' && file))} onClick={() => void start()}>{busy ? <LoaderCircle size={15} className="spin"/> : <ArrowUp size={16}/>}Start task<span className="shortcut">Ctrl ↵</span></button></div>
          </div>
          <div className="quick-actions">{quickActions.map(action => <button key={action.id} className={preset === action.id ? 'chosen' : ''} onClick={() => { setPreset(action.id); setRequest(action.text); setTimeout(() => input.current?.focus(), 0); }}><action.icon size={18}/><span><b>{action.title}</b><small>{action.description}</small></span><ArrowRight size={14}/></button>)}</div>
          <div className="project-context"><div className="section-label">IN THIS PROJECT</div><div className="context-line"><span><Folder size={14}/>{project.files.length.toLocaleString()} source files</span><span><GitBranch size={14}/>{project.git?.branch ?? 'Folder without Git'}</span><span className="stack-label">{project.stack === 'node' ? 'JavaScript / TypeScript' : project.stack === 'python' ? 'Python' : 'Source files'}</span></div><p title={project.path}>{project.path}</p><div className="project-context-actions"><button className="text-button" disabled={busy || !!running} onClick={() => setBrowserOpen(true)}>Browser checks<ArrowRight size={13}/></button><button className="text-button" onClick={() => { setPreset('review'); setRequest('Review the current project changes.'); }}>Inspect existing changes<ArrowRight size={13}/></button><button className="text-button" disabled={busy || !!running} onClick={() => void prepareHandoff()}>Prepare for Codex<ArrowRight size={13}/></button></div></div>
          {!selectedModel && <div className="model-hint"><Cpu size={17}/><p>Connect a model for coding tasks.<span>Search, checks and review work without one.</span></p><button className="text-button" onClick={() => setModelsOpen(true)}>Set up<ChevronRight size={14}/></button></div>}
        </div> : <div className="task-workspace"><div className="task-header"><div><span className="eyebrow">{task.route.level === 0 ? 'DETERMINISTIC WORKFLOW' : task.route.level === 4 ? 'CLOUD HANDOFF' : `LOCAL TASK · ${task.route.strategy.toUpperCase()}`}</span><h1>{task.request}</h1><div className="task-meta"><span className={`task-status ${task.state}`}>{activeTask(task) ? <LoaderCircle size={13} className="spin"/> : task.state === 'failed' ? <TriangleAlert size={13}/> : <CircleDot size={13}/>}<span aria-live="polite">{status}</span></span><span>{modeLabels[task.mode]}</span><span>{task.toolCalls} actions</span></div></div><div className="task-header-buttons">{activeTask(task) ? <button className="button" onClick={() => void api.cancel(task.id).catch(onError)}><Square size={13}/>Stop</button> : <button className="button" disabled={busy} onClick={() => void prepareHandoff()}><ArrowRight size={15}/>Prepare for Codex</button>}</div></div>
          {task.approval && <div className="approval-strip"><ShieldCheck size={17}/><span>{task.approval.title}</span><button className="button compact" onClick={() => setApprovalOpen(true)}>Review permission</button></div>}
          {task.changeState === 'recovery_required' && <div className="recovery-strip"><TriangleAlert size={17}/><span>A file operation was interrupted. Restore it before starting another task.</span><button className="button compact" onClick={() => setConfirm('recover')}>Restore files</button></div>}
          <nav className="task-tabs" aria-label="Task views">{[['activity', 'Activity'], ['changes', 'Changes'], ['checks', 'Checks']].map(([value, label]) => <button key={value} aria-current={tab === value ? 'page' : undefined} className={tab === value ? 'selected' : ''} onClick={() => setTab(value)}>{label}{value === 'changes' && !!review?.files.length && <span>{review.files.length}</span>}{value === 'checks' && !!task.checks.length && <span>{task.checks.length}</span>}</button>)}<div className="task-view-note">{task.model && <><Cpu size={13}/>{task.model}</>}</div></nav>
          <div className={`task-content ${tab}`}>{tab === 'activity' ? <Activity task={task}/> : tab === 'checks' ? <Checks task={task}/> : <ReviewPanel review={review} task={task} theme={resolvedTheme} busy={busy} onOperation={setConfirm}/>}</div>
          {!activeTask(task) && !['applying', 'undoing', 'recovery_required', 'superseded', 'discarded'].includes(task.changeState) && <><div className="task-image-history">{task.inputs?.filter(block => block.type === 'image').map(block => block.type === 'image' && <ImagePreview key={block.attachment_id} id={block.attachment_id}/>)}</div><div className="continue-composer"><textarea aria-label="Continue task" placeholder="Continue with a focused follow-up…" value={request} onChange={e => { setRequest(e.target.value); setPreset('task'); }} rows={1} onKeyDown={e => { if (e.ctrlKey && e.key === 'Enter') { e.preventDefault(); void start(task.id); } }}/><button className="icon-button primary" aria-label="Continue task" disabled={!request.trim() || busy || !!running} onClick={() => void start(task.id)}><ArrowUp size={17}/></button></div></>}
        </div>}
      </>}
      <footer className="workspace-footer"><span><span className={`status-dot ${running ? 'working' : ''}`}/>{running ? stateLabel[running.state] ?? 'Working' : 'Inference stays local'}</span><span>{project ? `${project.recipes.length} project checks available` : 'Windows workspace'}<span className="footer-divider"/><button className="text-button" aria-label="Local runtime help" onClick={() => void api.help('docs').catch(onError)}><HelpCircle size={13}/>Help</button></span></footer>
    </main>
    <ModelsDialog open={modelsOpen} onClose={() => { if (probing || calibrating) void api.cancel(calibrating ? 'model-calibration' : 'model-probe').catch(onError); setModelsOpen(false); }} runtime={runtime} settings={settings} loading={runtimeLoading} probing={probing} calibrating={calibrating} busy={busy || !!running} onRefresh={() => void refreshRuntime()} onUpdate={next => void saveSettings(next)} onProbe={name => { setProbing(name); setBusy(true); void api.probe(name).then(() => { setNotice('Model tool compatibility verified'); void refreshRuntime(); }).catch(onError).finally(() => { setProbing(null); setBusy(false); }); }} onCalibrate={name => { setCalibrating(name); setBusy(true); void api.calibrate(name).then(report => { setNotice(report.message); void refreshRuntime(); }).catch(onError).finally(() => { setCalibrating(null); setBusy(false); }); }} onError={onError}/>
    <SettingsDialog onModels={() => { setSettingsOpen(false); setModelsOpen(true); }} open={settingsOpen} onClose={() => setSettingsOpen(false)} settings={settings} storage={workspace.storage} onUpdate={next => void saveSettings(next)} onTool={kind => void api.chooseTool(kind).then(workspace.setSettings).catch(onError)} browserAvailable={workspace.browserAvailable} installing={installingBrowser} onInstallBrowser={() => { setInstallingBrowser(true); void api.installBrowser().then(() => { workspace.setBrowserAvailable(true); setNotice('Browser tools installed'); }).catch(onError).finally(() => setInstallingBrowser(false)); }}/>
    <BrowserDialog open={browserOpen} onClose={() => setBrowserOpen(false)} available={workspace.browserAvailable} busy={busy || !!running} onSetup={() => { setBrowserOpen(false); setSettingsOpen(true); }} onStart={spec => { if (!project) return; setBusy(true); void api.startTask(project.id, JSON.stringify(spec), 'browser', null, mode, null).then(created => { workspace.upsertTask(created); workspace.selectTask(created.id); setBrowserOpen(false); setTab('activity'); }).catch(onError).finally(() => setBusy(false)); }}/>
    <ApprovalDialog approval={approvalOpen ? task?.approval ?? null : null} busy={busy} onClose={() => setApprovalOpen(false)} onAnswer={choice => { if (!task?.approval) return; setBusy(true); void api.answer(task.id, task.approval.id, choice).then(() => setApprovalOpen(false)).catch(onError).finally(() => setBusy(false)); }}/>
    <HandoffDialog preview={handoff} busy={busy} onClose={() => setHandoff(null)} onError={onError} onExport={() => { if (!handoff) return; setBusy(true); void api.exportHandoff(handoff.id).then(path => { if (path) { setNotice(`Handoff exported to ${path}`); setHandoff(null); } }).catch(onError).finally(() => setBusy(false)); }}/>
    <Dialog open={confirm !== null} onClose={() => setConfirm(null)} title={confirm === 'apply' ? 'Apply these changes to your project?' : confirm === 'undo' ? 'Undo the applied changes?' : confirm === 'discard' ? 'Discard this proposal?' : 'Restore the interrupted file operation?'} description={confirm === 'apply' ? 'The exact reviewed files will be updated. Existing user changes are preserved, and a recovery backup is retained.' : confirm === 'undo' ? 'Files return to their pre-apply content only if they still match the applied version. Newer edits are never overwritten.' : confirm === 'discard' ? 'The proposal will no longer be available to apply. Your original project is unchanged.' : 'Known file versions will be restored from the durable recovery journal. Unrecognized content is preserved for manual review.'}><div className="dialog-body">{confirm === 'apply' && (!task?.checks.length || task.checks.some(c => c.status !== 'passed')) && <div className="confirmation-warning"><TriangleAlert size={18}/><p>Some checks failed, are stale, or have not been run. Review the Checks tab before applying this proposal.</p></div>}</div><div className="dialog-footer"><button className="button" data-neutral-focus onClick={() => setConfirm(null)}>Cancel</button><button className={`button ${confirm === 'discard' ? 'destructive' : 'primary'}`} disabled={busy} onClick={() => void performOperation()}>{confirm === 'apply' ? 'Apply changes' : confirm === 'undo' ? 'Undo changes' : confirm === 'discard' ? 'Discard proposal' : 'Restore files'}</button></div></Dialog>
  </div>;
}

