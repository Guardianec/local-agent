import { useEffect, useRef } from 'react';
import type { FileDelta } from '../../generated/FileDelta';
import type * as Monaco from 'monaco-editor';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';

self.MonacoEnvironment = { getWorker: () => new EditorWorker() };
const language = (path: string) => ({ ts: 'typescript', tsx: 'typescript', js: 'javascript', jsx: 'javascript', json: 'json', py: 'python', rs: 'rust', css: 'css', html: 'html', md: 'markdown' }[path.split('.').pop() ?? ''] ?? 'plaintext');
export default function DiffEditor({ file, split, theme }: { file: FileDelta; split: boolean; theme: string }) {
  const container = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let cancelled = false;
    let editor: Monaco.editor.IStandaloneDiffEditor | undefined;
    let before: Monaco.editor.ITextModel | undefined;
    let after: Monaco.editor.ITextModel | undefined;
    const observer = new ResizeObserver(() => editor?.layout());
    const element = container.current;
    if (!element) return;
    observer.observe(element);
    void import('monaco-editor/esm/vs/editor/editor.api').then(monaco => {
      if (cancelled) return;
      editor = monaco.editor.createDiffEditor(element, { readOnly: true, originalEditable: false, renderSideBySide: split, theme: theme === 'light' ? 'vs' : 'vs-dark', automaticLayout: false, minimap: { enabled: false }, fontSize: 13, lineHeight: 21, fontFamily: "'Cascadia Code', Consolas, monospace", scrollBeyondLastLine: false, renderOverviewRuler: false, padding: { top: 16 }, wordWrap: 'on', stickyScroll: { enabled: false }, diffCodeLens: false, accessibilitySupport: 'on', ariaLabel: `Proposed changes to ${file.path}` });
      before = monaco.editor.createModel(file.before, language(file.path)); after = monaco.editor.createModel(file.after, language(file.path));
      editor.setModel({ original: before, modified: after }); editor.layout();
    });
    return () => { cancelled = true; observer.disconnect(); editor?.dispose(); before?.dispose(); after?.dispose(); };
  }, [file, split, theme]);
  return <div className="diff-editor" ref={container}/>;
}
