import { Compartment, EditorState, StateField, type Extension } from '@codemirror/state';
import {
  Decoration,
  drawSelection,
  EditorView,
  keymap,
  placeholder,
  type DecorationSet,
} from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { taskKeymap, tasks } from './checkbox';

// Defined here rather than in styles.css: CodeMirror's base theme outranks plain CSS.
const theme = EditorView.theme({
  '&': { height: '100%', fontSize: '15px' },
  '&.cm-focused': { outline: 'none' },
  '.cm-scroller': { fontFamily: 'var(--font)', lineHeight: '1.6', padding: '12px 0 48px' },
  '.cm-content': { padding: '8px 28px', caretColor: 'var(--text)' },
  '.cm-placeholder': { color: 'var(--text-muted)' },
  // The theme decides the cursor: a thin line, or a block in Terminal.
  '.cm-cursor, .cm-dropCursor': {
    borderLeftWidth: 'var(--cursor-width)',
    borderLeftColor: 'var(--accent)',
  },
  // drawSelection paints the selection itself, so the theme can colour it.
  '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground':
    { background: 'var(--selection)' },
  '.cm-title-line': {
    fontSize: 'var(--title-size)',
    fontWeight: 'var(--title-weight)',
    lineHeight: '1.4',
    paddingBottom: '4px',
  },
});

const titleLine = Decoration.line({ class: 'cm-title-line' });

/** The first line is the note's title (the list uses it), so it looks like one. */
const title: Extension = StateField.define<DecorationSet>({
  create: (state) => Decoration.set([titleLine.range(state.doc.line(1).from)]),
  update: (deco, tr) =>
    tr.docChanged ? Decoration.set([titleLine.range(tr.state.doc.line(1).from)]) : deco,
  provide: (f) => EditorView.decorations.from(f),
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
        // CodeMirror's own cursor, so themes can make it a block (Terminal, Retro).
        drawSelection({ cursorBlinkRate: 1000 }),
        theme,
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: 'true', 'aria-label': 'Note text' }),
        placeholder('First line is the title. Type [] for a task.'),
        tasks,
        title,
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
