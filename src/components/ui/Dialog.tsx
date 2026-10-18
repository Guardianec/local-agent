import * as Primitive from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import { useRef, type ReactNode } from 'react';
export function Dialog({ open, title, description, children, onClose, wide = false }: { open: boolean; title: string; description?: string; children: ReactNode; onClose: () => void; wide?: boolean }) {
  const content = useRef<HTMLDivElement>(null);
  return <Primitive.Root open={open} onOpenChange={value => { if (!value) onClose(); }}><Primitive.Portal><Primitive.Overlay className="dialog-overlay"/><Primitive.Content ref={content} className={`dialog ${wide ? 'dialog-wide' : ''}`} onOpenAutoFocus={e => { e.preventDefault(); content.current?.querySelector<HTMLButtonElement>('[data-neutral-focus]')?.focus(); }}>
    <div className="dialog-heading"><div><Primitive.Title>{title}</Primitive.Title><Primitive.Description className={description ? '' : 'sr-only'}>{description ?? title}</Primitive.Description></div><Primitive.Close className="icon-button" aria-label="Close dialog"><X size={18}/></Primitive.Close></div>
    {children}
  </Primitive.Content></Primitive.Portal></Primitive.Root>;
}
