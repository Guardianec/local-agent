import { AlertTriangle, Check, ChevronDown, Circle, FilePenLine, Search, Shield, TerminalSquare } from 'lucide-react';
import type { Task } from '../../generated/Task';
const icons: Record<string, typeof Circle> = { warning: AlertTriangle, validation: Check, changes: FilePenLine, approval: Shield, tool: Search, usage: TerminalSquare };
export function Activity({ task }: { task: Task }) {
  return <div className="activity"><div className="activity-request"><span className="request-avatar">You</span><div><div className="eyebrow">TASK</div><p>{task.request}</p></div></div>
    <div className="timeline">{task.events.map(event => {
      const Icon = icons[event.kind] ?? Circle;
      return <div className={`timeline-event ${event.kind}`} key={event.id}><span className="timeline-dot"><Icon size={14}/></span><div className="timeline-content">{event.detail ? <details><summary><span>{event.title}</span><ChevronDown size={13}/><time>{new Date(event.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</time></summary><pre>{event.detail}</pre></details> : <div className="event-title">{event.title}<time>{new Date(event.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</time></div>}</div></div>;
    })}</div>
    {task.summary && <div className={`task-outcome ${task.state === 'failed' ? 'failed' : ''}`}><span className="eyebrow">{task.state === 'escalation_ready' ? 'READY FOR A HANDOFF' : 'TASK RESULT'}</span><p>{task.summary}</p>{task.warnings.map((w, i) => <p className="warning-text" key={i}>{w}</p>)}</div>}
  </div>;
}
export function Checks({ task }: { task: Task }) {
  if (!task.checks.length) return <div className="pane-empty"><TerminalSquare size={30}/><h3>No project checks yet</h3><p>Tests and linters run after you approve a registered project command.</p></div>;
  return <div className="checks">{task.checks.map(check => <details className="check-row" key={check.id} open={check.status !== 'passed'}><summary><span className={`status-icon ${check.status}`}>{check.status === 'passed' ? <Check size={16}/> : <AlertTriangle size={16}/>}</span><div><b>{check.name}</b><span>{check.status.replaceAll('_', ' ')}{check.status === 'stale' && check.outcomeStatus ? ` · previously ${check.outcomeStatus}` : ''} · {(check.durationMs / 1000).toFixed(1)}s{check.passed !== null ? ` · ${check.passed} passed${check.failed ? `, ${check.failed} failed` : ''}` : ' · test count unavailable'}</span></div><ChevronDown size={15}/></summary><div className="check-evidence"><p>{check.recipeId === 'browser' ? 'Approved page scenario · server effects are outside project Undo' : <>Tested source version <code>{check.manifestHash.slice(0, 12)}</code> · Exit {check.exitCode ?? 'unavailable'}</>}</p><pre>{check.output || 'No output was produced.'}</pre></div></details>)}</div>;
}

