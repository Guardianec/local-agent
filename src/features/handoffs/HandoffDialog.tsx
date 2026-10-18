import { Check, Clipboard, FolderOutput } from 'lucide-react';
import { useState } from 'react';
import type { HandoffPreview } from '../../generated/HandoffPreview';
import { Dialog } from '../../components/ui/Dialog';
export function HandoffDialog({ preview, busy, onClose, onExport, onError }: { preview: HandoffPreview | null; busy: boolean; onClose: () => void; onExport: () => void; onError: (e: unknown) => void }) {
  const [copied, setCopied] = useState(false);
  return <Dialog open={preview !== null} onClose={onClose} title="Prepare for Codex" description="Review the selected context before sharing it. Nothing is uploaded." wide><div className="dialog-body handoff-body"><div className="handoff-meta"><span>{((preview?.bytes ?? 0) / 1024).toFixed(1)} KB selected</span><span>{preview?.patch ? 'Complete patch included' : 'No applicable patch included'}</span></div><pre className="handoff-preview">{preview?.markdown}</pre><details className="omissions"><summary>Privacy and omitted content</summary>{preview?.omissions.map((line, i) => <p key={i}>{line}</p>)}</details></div><div className="dialog-footer"><button className="button" data-neutral-focus onClick={onClose}>Close</button><div><button className="button" disabled={!preview} onClick={() => { if (preview) void navigator.clipboard.writeText(preview.markdown).then(() => setCopied(true)).catch(onError); }}>{copied ? <Check size={15}/> : <Clipboard size={15}/>}Copy prompt</button><button className="button primary" disabled={busy} onClick={onExport}><FolderOutput size={15}/>Export folder</button></div></div></Dialog>;
}
