import { lazy, Suspense, useState } from 'react';
import { AlertTriangle, ArrowDownToLine, Check, Columns2, FileCode2, List, RotateCcw, Trash2 } from 'lucide-react';
import type { Review } from '../../generated/Review';
import type { Task } from '../../generated/Task';
const DiffEditor = lazy(() => import('./DiffEditor'));
export function ReviewPanel({ review, task, theme, busy, onOperation }: { review: Review | null; task: Task; theme: string; busy: boolean; onOperation: (operation: string) => void }) {
  const [path, setPath] = useState<string | null>(null);
  const [split, setSplit] = useState(true);
  const file = review?.files.find(f => f.path === path) ?? review?.files[0];
  if (!review) return <div className="pane-empty">Preparing the file comparison…</div>;
  if (!file) return <div className="pane-empty"><Check size={30}/><h3>No proposed file changes</h3><p>This task has not changed source files. Check Activity and Checks for its results.</p></div>;
  const added = review.files.reduce((sum, f) => sum + f.additions, 0), removed = review.files.reduce((sum, f) => sum + f.deletions, 0);
  return <section className="review-panel">
    <div className="review-summary"><div><b>{review.files.length} files changed</b><span className="added">+{added}</span><span className="removed">−{removed}</span></div><span className="muted">Task baseline → proposal</span></div>
    {review.warnings.length > 0 && <div className="review-notice"><AlertTriangle size={15}/><span>{review.warnings.join(' ')}</span></div>}
    <div className="diff-layout"><div className="changed-file-list" aria-label="Changed files">{review.files.map(f => <button key={f.path} className={file.path === f.path ? 'selected' : ''} onClick={() => setPath(f.path)}><FileCode2 size={15}/><span title={f.path}>{f.path}</span><small className={f.kind === 'deleted' ? 'removed' : f.kind === 'added' ? 'added' : ''}>{f.kind === 'added' ? 'A' : f.kind === 'deleted' ? 'D' : 'M'}</small></button>)}</div>
      <div className="diff-pane"><div className="diff-toolbar"><span title={file.path}>{file.path}</span><button className="icon-button" aria-label={split ? 'Use unified diff' : 'Use split diff'} onClick={() => setSplit(!split)}>{split ? <List size={16}/> : <Columns2 size={16}/>}</button></div>
      {!file.eligible ? <div className="pane-empty"><AlertTriangle/><h3>Manual review required</h3><p>Binary or secret-bearing changes cannot be applied by this app.</p></div> : <Suspense fallback={<div className="pane-empty">Loading file comparison…</div>}><DiffEditor file={file} split={split && window.innerWidth > 1100} theme={theme}/></Suspense>}</div>
    </div>
    <div className="review-footer"><span className="muted">{task.changeState === 'applied' ? 'Applied to your project · recovery data retained' : task.changeState === 'undone' ? 'Your pre-task files have been restored' : 'Your project files are unchanged until Apply'}</span><div>
      {task.changeState === 'proposed' && <><button className="button subtle danger-text" disabled={busy} onClick={() => onOperation('discard')}><Trash2 size={15}/>Discard</button><button className="button primary" disabled={busy || !review.canApply} onClick={() => onOperation('apply')}><ArrowDownToLine size={15}/>Apply changes</button></>}
      {task.changeState === 'applied' && <button className="button" disabled={busy} onClick={() => onOperation('undo')}><RotateCcw size={15}/>Undo applied changes</button>}
      {task.changeState === 'recovery_required' && <button className="button primary" disabled={busy} onClick={() => onOperation('recover')}><RotateCcw size={15}/>Restore interrupted operation</button>}
    </div></div>
  </section>;
}
