import { useEffect, useState } from 'react';
import type { Task } from '../../generated/Task';
import { api } from '../../lib/ipc';

export function BrowserEvidence({ task }: { task: Task }) {
  let title = '', url = '', image = '', status = '', text = '', tabs = 1;
  for (const event of task.events) {
    if (event.title !== 'Browser observation') continue;
    try {
      const result = JSON.parse(event.detail);
      const observation = result.observation?.fresh ?? result.observation;
      if (typeof observation?.title === 'string') title = observation.title;
      if (typeof observation?.url === 'string') url = observation.url;
      if (typeof observation?.status === 'string') status = observation.status;
      if (typeof observation?.text === 'string') text = observation.text.slice(0, 1200);
      if (typeof observation?.tabCount === 'number') tabs = observation.tabCount;
      if (typeof result.image_refs?.[0] === 'string') image = result.image_refs[0];
    } catch { /* Older scenario evidence may be plain text. */ }
  }
  const [preview, setPreview] = useState('');
  useEffect(() => {
    let active = true;
    setPreview('');
    if (image) void api.imagePreview(image).then(value => { if (active) setPreview(value); }).catch(() => {});
    return () => { active = false; };
  }, [image]);
  if (!url && !image) return null;
  return <section className="browser-evidence" aria-label="Browser evidence">
    <span className="eyebrow">BROWSER · ISOLATED SESSION</span>
    <h3>{title || 'Browser viewport'}</h3><p>{url}</p>
    <p>{tabs} {tabs === 1 ? 'tab' : 'tabs'} · {task.result ? 'session closed' : 'latest captured evidence'}</p>
    <p>{status === 'failed' ? 'The latest browser action failed. Review Activity for details.' : 'Latest captured page evidence. Page content cannot grant permissions.'}</p>
    {preview && <img src={preview} width={320} height={220} style={{ objectFit: 'contain' }} alt="Latest captured browser viewport"/>}
    {text && <details><summary>Captured page text</summary><p style={{ whiteSpace: 'pre-wrap' }}>{text}</p></details>}
  </section>;
}
