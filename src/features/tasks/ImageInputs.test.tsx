import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ImageInputs, ingestFiles } from './ImageInputs';
import { api } from '../../lib/ipc';
vi.mock('../../lib/ipc', () => ({ api: { imagePreview: vi.fn().mockResolvedValue('data:image/jpeg;base64,AA=='), chooseImages: vi.fn().mockResolvedValue([]), ingestImage: vi.fn() } }));
afterEach(cleanup);
describe('local image input', () => {
  it('previews and removes a reference before execution', () => {
    const change = vi.fn();
    render(<ImageInputs images={[{ id: 'image', name: 'UI.png', blob: 'hash', originalBlob: 'original', width: 80, height: 60, bytes: 100, resized: false, source: 'clipboard' }]} busy={false} onChange={change} onError={vi.fn()}/>);
    fireEvent.click(screen.getByRole('button', { name: 'Remove UI.png' }));
    expect(change).toHaveBeenCalledWith([]);
  });
  it('rejects excessive size and count before the native bridge', async () => {
    await expect(ingestFiles(Array.from({ length: 5 }, () => new File(['x'], 'UI.png')), 'drop')).rejects.toMatchObject({ code: 'IMAGE_COUNT' });
    await expect(ingestFiles([new File([new Uint8Array(8 * 1024 * 1024 + 1)], 'UI.png')], 'clipboard')).rejects.toMatchObject({ code: 'IMAGE_SIZE' });
    expect(api.ingestImage).not.toHaveBeenCalled();
  });
});
