import { Compartment, EditorState } from '@codemirror/state';
import { EditorView, keymap, placeholder } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { taskKeymap, tasks } from './checkbox';

// Defined here rather than in styles.css: CodeMirror's base theme outranks plain CSS.
const theme = EditorView.theme({
  '&': { height: '100%', fontSize: '15px' },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': { fontFamily: 'var(--font)', lineHeight: '1.6', padding: '12px 0 48px' },
  '.cm-content': { padding: '8px 28px', caretColor: 'var(--text)' },
  '.cm-placeholder': { color: 'var(--text-muted)' },
});

export interface EditorHandle {
  focus(): void;
  destroy(): void;
}

/** Minimal CodeMirror: history, keymap, wrapping, tasks; markdown loads lazily. */
export function createEditor(
  parent: HTMLElement,
  doc: string,
  onChange: (doc: string) => void,
): EditorHandle {
  const language = new Compartment();
  let destroyed = false;

  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        history(),
        theme,
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: 'true', 'aria-label': 'Note text' }),
        placeholder('First line is the title. Type [] for a task.'),
        tasks,
        keymap.of([...taskKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
        language.of([]),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) onChange(u.state.doc.toString());
        }),
      ],
    }),
  });

  import('./markdown').then(({ markdownSupport }) => {
    if (!destroyed) view.dispatch({ effects: language.reconfigure(markdownSupport) });
  });

  return {
    focus: () => view.focus(),
    destroy: () => {
      destroyed = true;
      view.destroy();
    },
  };
}
