import { useEffect, useState } from 'react';
import { CheckCircle2, Cpu, Download, ExternalLink, LoaderCircle, RefreshCw, ServerOff } from 'lucide-react';
import type { RuntimeInfo } from '../../generated/RuntimeInfo';
import type { Settings } from '../../generated/Settings';
import { ModelSetup } from './ModelSetup';
import { Dialog } from '../../components/ui/Dialog';
import { api } from '../../lib/ipc';

type Props = {
  open: boolean; onClose: () => void; runtime: RuntimeInfo | null; settings: Settings;
  loading: boolean; probing: string | null; calibrating: string | null; busy: boolean;
  onRefresh: () => void; onUpdate: (settings: Settings) => void;
  onProbe: (name: string) => void; onCalibrate: (name: string) => void; onError: (e: unknown) => void;
};
const tiers = [
  ['fast', 'Fast', 'Simple edits and repetitive tasks'],
  ['balanced', 'Balanced', 'Moderate debugging and multi-file work'],
  ['strong', 'Strong', 'Confirmed, bounded attempts with a calibrated model'],
] as const;
export function ModelsDialog({ open, onClose, runtime, settings, loading, probing, calibrating, busy, onRefresh, onUpdate, onProbe, onCalibrate, onError }: Props) {
  const [profiles, setProfiles] = useState(settings.modelProfiles);
  const [confirm, setConfirm] = useState<string | null>(null);
  useEffect(() => { setProfiles(settings.modelProfiles); }, [open, settings.modelProfiles]);
  const reports = runtime?.calibrations ?? [];
  const contextFor = (name: string) => name === profiles.strong ? settings.intelligence.strongContext : name === profiles.balanced ? settings.intelligence.balancedContext : settings.intelligence.fastContext;
  const reportFor = (name: string) => {
    const model = runtime?.models.find(m => m.name === name);
    return reports.find(c => c.digest === model?.digest && c.runtime === runtime?.version && c.contextSize === contextFor(name));
  };
  const strongReport = profiles.strong ? reportFor(profiles.strong) : undefined;
  const strongModel = runtime?.models.find(m => m.name === profiles.strong);
  const strongReady = strongModel?.capability === 'verified' && strongReport?.passed;
  const working = probing || calibrating;
  return <>
    <Dialog open={open} onClose={onClose} wide title="Local models" description="Three profiles. One local model at a time."><div className="dialog-body">
      <div className="runtime-connection"><span className={`connection-icon ${runtime?.status === 'ready' ? 'connected' : ''}`}><Cpu size={23}/></span><div><b>Ollama</b><span>{runtime?.status === 'ready' ? `Connected · ${runtime.version ?? 'local runtime'}` : 'Local model service is offline'}</span></div><button className="icon-button" aria-label="Refresh runtime" onClick={onRefresh} disabled={loading}><RefreshCw className={loading ? 'spin' : ''} size={17}/></button></div>
      {runtime?.status !== 'ready' ? <div className="setup-state"><ServerOff size={30}/><h3>Connect your local runtime</h3><p>Install or open Ollama, then refresh. Search and review work without a model.</p><button className="button primary" onClick={() => void api.help('download').catch(onError)}><Download size={15}/>Get Ollama<ExternalLink size={13}/></button></div> : <>
        <ModelSetup open={open} runtime={runtime} settings={settings} busy={busy || !!working} onUpdate={onUpdate} onRefresh={onRefresh} onError={onError}/><section className="model-profiles"><div className="section-label">AUTOMATIC TASK ROUTING</div>
          {tiers.map(([key, title, purpose]) => <label className="profile-assignment" key={key}><span><b>{title}</b><small>{purpose}</small></span><select aria-label={`${title} model`} value={profiles[key] ?? ''} disabled={busy || !!working} onChange={e => setProfiles({ ...profiles, [key]: e.target.value || null, ...(key === 'strong' ? { strongEnabled: false } : {}) })}><option value="">Not assigned</option>{profiles[key] && !runtime.models.some(m => m.name === profiles[key]) && <option value={profiles[key]!}>{profiles[key]} · unavailable</option>}{runtime.models.filter(m => m.local).map(m => <option key={m.name} value={m.name}>{m.name}{m.capability !== 'verified' ? ' · check tools' : ''}</option>)}</select></label>)}
          <label className="strong-toggle"><input type="checkbox" checked={profiles.strongEnabled && !!strongReady} disabled={!strongReady || busy || !!working} onChange={e => setProfiles({ ...profiles, strongEnabled: e.target.checked })}/><span>Enable Strong after confirmation<small>{strongReady ? 'Coding benchmark passed at this context size; RAM offload is allowed.' : 'Run the Strong coding benchmark at its selected context size. Available memory is checked before loading.'}</small></span></label>
          <div className="profile-footer"><span>Deterministic tasks use no model. Missing Fast falls back to Balanced.</span><button className="button compact" disabled={busy || !!working || JSON.stringify(profiles) === JSON.stringify(settings.modelProfiles)} onClick={() => onUpdate({ ...settings, model: profiles.balanced, modelProfiles: { ...profiles, strongEnabled: profiles.strongEnabled && !!strongReady } })}>Save profiles</button></div>
        </section>
        {runtime.gpu && <div className="gpu-status"><Cpu size={15}/><span>{runtime.gpu.name}</span><b>{(runtime.gpu.freeBytes / 1073741824).toFixed(1)} / {(runtime.gpu.totalBytes / 1073741824).toFixed(1)} GiB free</b></div>}
        {runtime.memory && <div className="gpu-status"><Cpu size={15}/><span>System RAM available</span><b>{(runtime.memory.availableBytes / 1073741824).toFixed(1)} / {(runtime.memory.totalBytes / 1073741824).toFixed(1)} GiB</b></div>}
        {!runtime.models.length ? <div className="setup-state"><h3>No local models installed</h3><p>Download a tool-capable model using Ollama, then refresh this list.</p><button className="button" onClick={() => void api.help('models').catch(onError)}>Browse models<ExternalLink size={13}/></button></div> : <div className="model-list"><div className="section-label">INSTALLED ON THIS PC</div>{runtime.models.map(model => {
          const report = reportFor(model.name);
          return <div className="model-row" key={model.name}><div className="model-select"><Cpu size={17}/><div><b>{model.name}</b><span>{model.parameterSize} · {model.quantization} · {(model.size / 1e9).toFixed(1)} GB on disk</span></div>{model.loaded && <small className="loaded-label">Loaded</small>}</div>
            <div className="model-capability"><span>{model.capability === 'verified' ? <><CheckCircle2 size={13}/>Tools verified</> : !model.local ? 'Remote model unavailable' : 'Tool compatibility not checked'}</span><div className="model-actions">{model.capability !== 'verified' && <button className="text-button" disabled={busy || !!working || !model.local} onClick={() => onProbe(model.name)}>Check tools</button>}<button className="text-button" aria-label="Test coding" title="Run local model benchmark" disabled={busy || !!working || !model.local} onClick={() => setConfirm(model.name)}>{calibrating === model.name ? <><LoaderCircle className="spin" size={13}/>Testing…</> : 'Test coding'}</button></div></div>
            {report && <details className={`calibration-result ${report.passed ? 'passed' : 'failed'}`}><summary>{report.passed ? 'Coding test passed' : 'Coding test needs attention'} · {report.contextSize.toLocaleString()} context · {(report.elapsedMs / 1000).toFixed(1)}s{report.tokensPerSecond !== null ? ` · ${report.tokensPerSecond.toFixed(1)} tokens/s` : ''}</summary><p>{report.message}</p><p>GPU allocation: {report.allocationBytes !== null ? `${(report.allocationBytes / 1073741824).toFixed(2)} GiB` : 'unknown'} · {report.fullGpu === true ? 'fully on GPU' : report.fullGpu === false ? 'GPU + system RAM (supported)' : 'GPU placement unknown'} · measured free: {report.freeGpuBytes !== null ? `${(report.freeGpuBytes / 1073741824).toFixed(2)} GiB` : 'unknown'}</p><p>First token after preparation: {report.timeToFirstTokenMs !== null ? `${(report.timeToFirstTokenMs / 1000).toFixed(2)}s` : 'not reported'} · coding retries: {report.retries} · escalations: {report.escalations}</p><p>Minimum available system RAM: {report.minimumFreeRamBytes !== null ? `${(report.minimumFreeRamBytes / 1073741824).toFixed(2)} GiB` : 'not sampled'} · peak total system RAM used: {report.peakRamBytes !== null ? `${(report.peakRamBytes / 1073741824).toFixed(2)} GiB` : 'not sampled'}</p><pre>{report.evidence}</pre></details>}
          </div>;
        })}</div>}
      </>}
      <details className="model-cleanup"><summary>Advanced benchmark</summary><p>The built-in fixture measures tool use, coding success, load/task time, first token, throughput and sampled memory. Other models can be tested individually above.</p><button className="button compact" disabled={busy || !!working || !profiles.balanced} onClick={() => setConfirm(profiles.balanced)}>Run local model benchmark</button></details><div className="quiet-note">Inference stays local. Tasks release their selected model afterward. Ollama is shared; avoid running another inference app during calibration.</div>
    </div><div className="dialog-footer">{working ? <button className="button" data-neutral-focus onClick={() => void api.cancel(calibrating ? 'model-calibration' : 'model-probe').catch(onError)}>Stop {calibrating ? 'coding test' : 'compatibility check'}</button> : <span className="muted small">Contexts are configured per tier in Advanced details</span>}<button className="button" data-neutral-focus onClick={onClose}>Done</button></div></Dialog>
    <Dialog open={confirm !== null} onClose={() => setConfirm(null)} title="Run local model benchmark?" description="A disposable built-in Node.js project checks real tool use, an edit and actual tests. Your project is not changed."><div className="dialog-body"><p className="calibration-warning">The fixture runs model-generated JavaScript with your Windows account. This is host execution, not a sandbox. Fixed fixture command approvals are covered by starting this test.</p><p className="muted small">Model: {confirm} · Context: {(confirm ? contextFor(confirm) : 0).toLocaleString()} · up to four minutes plus compatibility checks. Keep other GPU work quiet for useful memory measurements.</p></div><div className="dialog-footer"><button className="button" data-neutral-focus onClick={() => setConfirm(null)}>Cancel</button><button className="button primary" onClick={() => { const name = confirm; setConfirm(null); if (name) onCalibrate(name); }}>Start coding test</button></div></Dialog>
  </>;
}

