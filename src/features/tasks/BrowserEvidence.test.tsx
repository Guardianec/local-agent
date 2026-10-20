import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { Task } from '../../generated/Task';
import { BrowserEvidence } from './BrowserEvidence';

describe('browser evidence', () => {
  it('shows bounded page evidence and ignores malformed legacy records', () => {
    const task = { events: [{ title: 'Browser observation', detail: 'legacy text' }, { title: 'Browser observation', detail: JSON.stringify({ observation: { title: 'Fixture page', url: 'https://example.com/docs', status: 'passed', tabCount: 2 } }) }] } as Task;
    render(<BrowserEvidence task={task}/>);
    expect(screen.getByRole('region', { name: 'Browser evidence' })).toBeTruthy();
    expect(screen.getByText('Fixture page')).toBeTruthy();
    expect(screen.getByText('https://example.com/docs')).toBeTruthy();
    expect(screen.getByText(/2 tabs/)).toBeTruthy();
  });
});
