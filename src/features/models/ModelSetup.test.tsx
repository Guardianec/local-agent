import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import registry from '../../../assets/model-registry.json';
import { ModelSetup } from './ModelSetup';
import { api } from '../../lib/ipc';
import { useWorkspace } from '../../state/workspace';
vi.mock('../../lib/ipc', () => ({ api: { catalog: vi.fn(), cleanupCandidates: vi.fn(), cancel: vi.fn().mockResolvedValue(undefined), installModel: vi.fn(), installProgress: vi.fn().mockResolvedValue(null), removeModel: vi.fn().mockResolvedValue(undefined) }, errorOf: (e: unknown) => e }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
function mount() {
  vi.mocked(api.catalog).mockResolvedValue(registry.tiers);
  vi.mocked(api.cleanupCandidates).mockResolvedValue([{ name: 'old-app', owned: true, size: 1, reason: 'Matching receipt' }, { name: 'old-custom', owned: false, size: 1, reason: 'Ambiguous origin' }]);
  return render(<ModelSetup open runtime={{ status: 'ready', installed: true, models: [], calibrations: [], version: '1', message: '', gpu: null, memory: null }} settings={useWorkspace.getState().settings} busy={false} onUpdate={vi.fn()} onRefresh={vi.fn()} onError={vi.fn()}/>);
}
describe('explicit local model management', () => {
  it('requires confirmation for the large download', async () => {
    mount(); const buttons = await screen.findAllByRole('button', { name: 'Install', exact: true });
    fireEvent.click(buttons[2]);
    expect(screen.getByRole('dialog')).toHaveTextContent('24.0 GB');
    expect(api.installModel).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel', exact: true }));
  });
  it('cancels a running download when its setup view closes', async () => {
    vi.mocked(api.installModel).mockImplementation(() => new Promise(() => {}));
    const view = mount(); fireEvent.click((await screen.findAllByRole('button', { name: 'Install', exact: true }))[0]);
    fireEvent.click(screen.getByRole('button', { name: 'Download model' }));
    await waitFor(() => expect(api.installModel).toHaveBeenCalledWith('fast'));
    view.unmount(); expect(api.cancel).toHaveBeenCalledWith('model-install');
  });
  it('preselects only owned models and does not remove before confirmation', async () => {
    mount(); await screen.findByText('Model cleanup · 2 superseded');
    fireEvent.click(screen.getByText('Model cleanup · 2 superseded'));
    expect(screen.getByLabelText(/old-app/)).toBeChecked();
    expect(screen.getByLabelText(/old-custom/)).not.toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: 'Remove selected models' }));
    expect(api.removeModel).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Remove models', exact: true }));
    await waitFor(() => expect(api.removeModel).toHaveBeenCalledWith('old-app'));
    expect(api.removeModel).toHaveBeenCalledTimes(1);
  });
});
