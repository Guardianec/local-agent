import { useEffect, useState } from 'react';
import { api, errorOf } from '../../lib/ipc';
import type { Integration } from '../../generated/Integration';

type Report = { status: string; protocol: string; capability: string; tools: { name: string; effect: string }[] };
export function Integrations() {
  const [items, setItems] = useState<Integration[]>([]);
  const [editing, setEditing] = useState<Integration | null>(null);
  const [secret, setSecret] = useState('');
  const [clearSecret, setClearSecret] = useState(false);
  const [statuses, setStatuses] = useState<Awaited<ReturnType<typeof api.integrationStatus>>>({});
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [reports, setReports] = useState<Record<string, Report>>({});
  useEffect(() => () => { void api.cancel('integration-test').catch(() => {}); }, []);
  useEffect(() => { let alive = true; Promise.all([api.integrations(), api.integrationStatus()]).then(([value, status]) => { if (alive) { setItems(value); setStatuses(status); } }).catch(e => { if (alive) setError(errorOf(e).message); }); return () => { alive = false; }; }, []);
  async function run(action: () => Promise<void>) {
    setBusy(true); setError('');
    try { await action(); } catch (e) { setError(errorOf(e).message); } finally { try { setItems(await api.integrations()); setStatuses(await api.integrationStatus()); } catch (e) { setError(errorOf(e).message); } setBusy(false); }
  }
  function add(preset: string) {
    setSecret(''); setClearSecret(false); setConfirmed(false);
    setEditing({ id: crypto.randomUUID(), name: preset === 'github' ? 'GitHub' : preset === 'context7' ? 'Context7' : 'Custom tools', preset, enabled: true, approved: false, credential: null,
      transport: { type: 'http', local: false, url: preset === 'github' ? 'https://api.githubcopilot.com/mcp/' : preset === 'context7' ? 'https://mcp.context7.com/mcp' : '', headers: {} } });
  }
  function update(value: Integration) { setEditing(value); setConfirmed(false); }
  return <section className="integrations"><h3>Integrations</h3><p className="muted">Connect tools when a task needs them. Connections are lazy; every call has a separate data and action review.</p>
    <div className="integration-actions">{['github', 'context7', 'custom'].map(preset => <button key={preset} className="button compact" disabled={busy} onClick={() => add(preset)}>Add {preset === 'github' ? 'GitHub' : preset === 'context7' ? 'Context7' : 'custom MCP'}</button>)}</div>
    {error && <p role="alert">{error}</p>}
    {items.map(item => <article className="integration-card" key={item.id}><b>{item.name}</b><p className="muted">{item.enabled ? 'Enabled · connects on demand' : 'Disabled'} · {item.preset === 'custom' ? 'Unclassified effects · Act tasks only' : 'Read-only tools'} · {item.credential ? 'Protected credential saved' : 'No credential'}</p>
      {statuses[item.id] && <div><p className="muted">Last check: {statuses[item.id].state.replaceAll('_', ' ')} · Authentication: {statuses[item.id].authentication.replaceAll('_', ' ')} · {statuses[item.id].tools.length} available tools{statuses[item.id].withheld ? ` · ${statuses[item.id].withheld} withheld (unsupported or sensitive schemas)` : ''}</p>{statuses[item.id].lastError && <p role="alert">{statuses[item.id].lastError}</p>}</div>}
      <div className="integration-actions"><button className="button compact" disabled={busy || !item.enabled} onClick={() => void run(async () => { const report = await api.testIntegration(item.id); setReports(previous => ({ ...previous, [item.id]: report })); })}>Test / reconnect</button><button className="button compact" disabled={busy} onClick={() => { setEditing(item); setConfirmed(false); setClearSecret(false); setSecret(''); }}>Configure</button><button className="button compact" disabled={busy} onClick={() => void run(async () => { await api.saveIntegration({ ...item, enabled: !item.enabled }, null, true); setReports(previous => { const next = { ...previous }; delete next[item.id]; return next; }); })}>{item.enabled ? 'Disable' : 'Enable'}</button><button className="button compact" disabled={busy} onClick={() => void run(async () => { await api.removeIntegration(item.id); })}>Remove</button></div>
      {reports[item.id] && <details><summary>Connection test passed · {reports[item.id].tools.length} tools · closed cleanly</summary><p>Protocol {reports[item.id].protocol} · {reports[item.id].capability}</p><ul>{reports[item.id].tools.map(tool => <li key={tool.name}>{tool.name} · {tool.effect}</li>)}</ul></details>}
    </article>)}
    {editing && <form className="integration-form" onSubmit={event => { event.preventDefault(); void run(async () => { await api.saveIntegration(editing, clearSecret ? '' : secret || null, confirmed); setSecret(''); setClearSecret(false); setEditing(null); }); }}>
      <h4>{editing.approved ? 'Configure integration' : 'Add integration'}</h4>
      <label>Name<input required maxLength={80} value={editing.name} onChange={e => update({ ...editing, name: e.target.value })}/></label>
      {editing.preset === 'custom' && <label>Transport<select value={editing.transport.type} onChange={e => update({ ...editing, transport: e.target.value === 'http' ? { type: 'http', url: '', local: false, headers: {} } : { type: 'stdio', executable: '', arguments: [], directory: '', environment: {} } })}><option value="http">Streamable HTTP</option><option value="stdio">Local stdio program</option></select></label>}
      {editing.transport.type === 'http' ? <><label>Server URL<input required readOnly={editing.preset !== 'custom'} value={editing.transport.url} onChange={e => { if (editing.transport.type === 'http') update({ ...editing, transport: { ...editing.transport, url: e.target.value } }); }}/></label>{editing.preset === 'custom' && <label><input type="checkbox" checked={editing.transport.local} onChange={e => { if (editing.transport.type === 'http') update({ ...editing, transport: { ...editing.transport, local: e.target.checked } }); }}/>Deliberately allow this loopback development endpoint</label>}</> : <><label>Installed executable (.exe)<input required value={editing.transport.executable} onChange={e => { if (editing.transport.type === 'stdio') update({ ...editing, transport: { ...editing.transport, executable: e.target.value } }); }}/></label><label>Arguments (one per line; no secrets)<textarea value={editing.transport.arguments.join('\n')} onChange={e => { if (editing.transport.type === 'stdio') update({ ...editing, transport: { ...editing.transport, arguments: e.target.value.split('\n').filter(Boolean) } }); }}/></label><label>Working directory<input required value={editing.transport.directory} onChange={e => { if (editing.transport.type === 'stdio') update({ ...editing, transport: { ...editing.transport, directory: e.target.value } }); }}/></label><label>Credential environment variable (optional)<input value={Object.keys(editing.transport.environment)[0] ?? ''} onChange={e => { if (editing.transport.type === 'stdio') update({ ...editing, transport: { ...editing.transport, environment: e.target.value ? { [e.target.value]: 'credential' } : {} } }); }}/></label></>}
      <label>{editing.credential ? 'Replace protected credential (blank keeps current)' : 'Protected API key or token (optional)'}<input type="password" autoComplete="new-password" value={secret} maxLength={4096} onChange={e => setSecret(e.target.value)}/></label>
      {editing.credential && <label><input type="checkbox" checked={clearSecret} onChange={e => setClearSecret(e.target.checked)}/>Remove the saved credential when saving</label>}
      {editing.preset === 'custom' && editing.transport.type === 'http' && <label>Credential header names (optional, one per line; blank uses Bearer)<textarea value={Object.keys(editing.transport.headers).join('\n')} onChange={e => { if (editing.transport.type === 'http') update({ ...editing, transport: { ...editing.transport, headers: Object.fromEntries(e.target.value.split('\n').map(value => value.trim()).filter(Boolean).map(name => [name, 'credential'])) } }); }}/></label>}
      <p className="muted">Credentials are encrypted for your Windows account. HTTP uses Bearer authentication. OAuth and server-provided credential prompts are not supported yet.</p>
      <label className="strong-toggle"><input required type="checkbox" checked={confirmed} onChange={e => setConfirmed(e.target.checked)}/><span>{editing.transport.type === 'stdio' ? 'I approve this exact command. This program runs on my computer with my user permissions; it is not sandboxed.' : 'I approve this endpoint and sending its configured credential when connecting.'}</span></label>
      <div className="integration-actions"><button className="button primary" disabled={busy || !confirmed} type="submit">{busy ? 'Saving…' : 'Save integration'}</button><button className="button" type="button" disabled={busy} onClick={() => { setEditing(null); setSecret(''); }}>Cancel</button></div>
    </form>}
    {busy && <p role="status">Working on this integration… <button className="button compact" onClick={() => void api.cancel('integration-test').catch(() => {})}>Stop connection test</button></p>}
  </section>;
}
