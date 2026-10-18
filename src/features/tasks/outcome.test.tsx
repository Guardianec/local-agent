import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { Task } from '../../generated/Task';
import { outcomeText, diagnostics } from './outcome';
import { Activity } from './Activity';

const task = { id: 'fixture', state: 'failed', summary: ' ', request: 'private prompt', warnings: [], events: [], checks: [], model: 'private-model', intent: 'inspect', toolCalls: 0 } as unknown as Task;
describe('visible terminal outcomes', () => {
  it('renders a fallback and retry even when backend summary is blank', () => {
    render(<Activity task={task} onRetry={() => {}}/>);
    expect(screen.getByText(/Something ended unexpectedly/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
    expect(screen.getByText('Technical details')).toBeTruthy();
  });
  it('uses structured result and hides intermediate summaries', () => {
    const t = { ...task, summary: 'old summary', result: { userMessage: 'Useful answer' } } as Task;
    expect(outcomeText(t)).toBe('Useful answer');
    expect(outcomeText({ ...t, state: 'running' })).toBe('');
  });
  it('copies only allowed metadata, never prompt, paths, source, images or error details', () => {
    const report = diagnostics({ ...task, events: [{ kind: 'warning', detail: 'private source', title: 'private path' }, { kind: 'state', detail: 'running → failed' }], inputs: [{ type: 'text', text: 'secret image reference' }] } as Task);
    for (const secret of ['private prompt', 'private-model', 'private source', 'private path', 'secret image']) expect(report).not.toContain(secret);
    expect(report).toContain('running → failed');
  });
});
