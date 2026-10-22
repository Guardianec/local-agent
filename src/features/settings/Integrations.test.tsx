import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { Integrations } from './Integrations';
const api = vi.hoisted(() => ({ integrations: vi.fn(), integrationStatus: vi.fn(), cancel: vi.fn().mockResolvedValue(undefined), saveIntegration: vi.fn(), removeIntegration: vi.fn(), testIntegration: vi.fn() }));
vi.mock('../../lib/ipc', () => ({ api, errorOf: (e: unknown) => ({ message: String(e) }) }));
beforeEach(() => { vi.clearAllMocks(); api.integrations.mockResolvedValue([]); api.integrationStatus.mockResolvedValue({}); api.saveIntegration.mockResolvedValue({}); });
afterEach(cleanup);
it('requires confirmation and keeps the credential outside persisted configuration', async () => {
  render(<Integrations/>);
  fireEvent.click(screen.getByRole('button', { name: 'Add GitHub' }));
  const save = screen.getByRole('button', { name: 'Save integration' });
  expect(save).toBeDisabled();
  fireEvent.change(screen.getByLabelText('Protected API key or token (optional)'), { target: { value: 'test-only-credential' } });
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(save);
  await waitFor(() => expect(api.saveIntegration).toHaveBeenCalledTimes(1));
  const [config, secret, confirmed] = api.saveIntegration.mock.calls[0];
  expect(JSON.stringify(config)).not.toContain('test-only-credential');
  expect(secret).toBe('test-only-credential');
  expect(confirmed).toBe(true);
});
it('revokes confirmation when the approved endpoint changes', async () => {
  render(<Integrations/>);
  fireEvent.click(screen.getByRole('button', { name: 'Add custom MCP' }));
  fireEvent.click(screen.getAllByRole('checkbox')[1]);
  expect(screen.getByRole('button', { name: 'Save integration' })).toBeEnabled();
  fireEvent.change(screen.getByLabelText('Server URL'), { target: { value: 'https://example.com/mcp' } });
  expect(screen.getByRole('button', { name: 'Save integration' })).toBeDisabled();
});
