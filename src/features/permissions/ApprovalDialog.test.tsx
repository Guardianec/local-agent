import { fireEvent,render,screen } from '@testing-library/react';
import { describe,it,expect,vi } from 'vitest';
import { ApprovalDialog } from './ApprovalDialog';
describe('approval user intent',()=>{
  it('does not approve when dismissed and only offers configured scope',()=>{
    const answer=vi.fn(),close=vi.fn();const approval={id:'a',taskId:'t',operationHash:'h',title:'Change a file?',detail:'Private copy',preview:'diff',risk:'project_change',expiresAt:'2026-10-16',allowTask:false};
    render(<ApprovalDialog approval={approval} busy={false} onAnswer={answer} onClose={close}/>);
    expect(screen.queryByRole('button',{name:'Allow for this task'})).not.toBeInTheDocument();fireEvent.click(screen.getByRole('button',{name:'Close dialog'}));expect(close).toHaveBeenCalledOnce();expect(answer).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'Deny'}));expect(answer).toHaveBeenCalledWith('deny');
  });
});
