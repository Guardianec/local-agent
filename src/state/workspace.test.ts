import { describe,it,expect } from 'vitest';
import { useWorkspace } from './workspace';
import type { Task } from '../generated/Task';
describe('durable task event projection',()=>{
  it('rejects older task events and deduplicates replay',()=>{
    const task:Task={id:'task',projectId:'project',request:'work',preset:'task',state:'running',changeState:'none',revision:5,mode:'assisted',createdAt:'2026-10-16',finishedAt:null,model:null,route:{level:1,score:0,strategy:'fast',reasons:[],estimatedFiles:1},summary:'',warnings:[],events:[],checks:[],approval:null,turns:0,toolCalls:0,commands:0,generatedTokens:0,modelEvalMs:0,parentTaskId:null,snapshotId:null};
    useWorkspace.setState({tasks:[]});useWorkspace.getState().upsertTask(task);useWorkspace.getState().upsertTask({...task,revision:2,state:'queued'});useWorkspace.getState().upsertTask(task);
    expect(useWorkspace.getState().tasks).toHaveLength(1);expect(useWorkspace.getState().tasks[0].revision).toBe(5);expect(useWorkspace.getState().tasks[0].state).toBe('running');
  });
});
