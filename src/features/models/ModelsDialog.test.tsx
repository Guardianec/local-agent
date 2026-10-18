import { fireEvent, render, screen, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ModelsDialog } from './ModelsDialog';
import type { Settings } from '../../generated/Settings';
import type { RuntimeInfo } from '../../generated/RuntimeInfo';

vi.mock('../../lib/ipc', () => ({ api: { catalog: vi.fn().mockResolvedValue([]), cleanupCandidates: vi.fn().mockResolvedValue([]), imagePreview: vi.fn().mockResolvedValue('data:image/jpeg;base64,AA==') } }));
afterEach(cleanup);
const settings: Settings = { intelligence:{registryVersion:1,automaticEscalation:true,fastContext:8192,balancedContext:8192,strongContext:8192},theme:'dark',model:'medium',contextSize:8192,modelProfiles:{ fast:'small',balanced:'medium',strong:'large',strongEnabled:false },nodePath:null,pythonPath:null,gitPath:null,lastProjectId:null };
function runtime(): RuntimeInfo {
  return {status:'ready',version:'1',installed:true,message:'Local',gpu:null,memory:null,calibrations:[],models:['small','medium','large'].map(name=>({name,digest:name,size:1,parameterSize:'8B',quantization:'Q4',loaded:false,vram:null,loadedBytes:null,local:true,vision:true,thinking:true,capability:'verified'}))};
}
function mount(health: RuntimeInfo) {
  const update=vi.fn(),calibrate=vi.fn();
  render(<ModelsDialog open onClose={vi.fn()} runtime={health} settings={settings} loading={false} probing={null} calibrating={null} busy={false} onRefresh={vi.fn()} onUpdate={update} onProbe={vi.fn()} onCalibrate={calibrate} onError={vi.fn()}/>);
  return {update,calibrate};
}
describe('model profile intent',()=>{
  it('does not enable Strong without current coding evidence',()=>{
    mount(runtime());
    expect(screen.getByRole('checkbox')).toBeDisabled();
    expect(screen.getByLabelText('Fast model')).toHaveValue('small');
    expect(screen.getByLabelText('Balanced model')).toHaveValue('medium');
    expect(screen.getByLabelText('Strong model')).toHaveValue('large');
  });
  it('requires confirmation before executing a coding calibration',()=>{
    const {calibrate}=mount(runtime());
    fireEvent.click(screen.getAllByRole('button',{name:'Test coding'})[0]);
    expect(calibrate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button',{name:'Start coding test'}));
    expect(calibrate).toHaveBeenCalledWith('small');
  });
  it('saves explicitly enabled Strong only with a matching calibration',()=>{
    const health=runtime();
    health.calibrations=[{model:'large',digest:'large',runtime:'1',contextSize:8192,passed:true,fullGpu:false,elapsedMs:1000,tokensPerSecond:20,allocationBytes:1,freeGpuBytes:1073741824,message:'Passed',evidence:'Actual tests',testedAt:'2026-10-16',peakRamBytes:null,minimumFreeRamBytes:null,timeToFirstTokenMs:null,retries:0,escalations:0}];
    const {update}=mount(health);
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(screen.getByRole('button',{name:'Save profiles'}));
    expect(update.mock.calls[0][0].modelProfiles.strongEnabled).toBe(true);
  });
});
