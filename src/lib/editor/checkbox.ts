import { RangeSetBuilder, type Line } from '@codemirror/state';
import {
  Decoration,
  EditorView,
  ViewPlugin,
  WidgetType,
  type Command,
  type DecorationSet,
  type KeyBinding,
  type ViewUpdate,
} from '@codemirror/view';

/** `- [ ] `, `* [x] `, `1. [ ] ` … Groups: indent, bullet, mark. */
const TASK_RE = /^(\s*)([-*+]|\d+[.)]) \[([ xX])\](?=\s|$)/;
/** Any list item: indent, bullet, optional task box. */
const ITEM_RE = /^(\s*)([-*+]|\d+[.)]) (\[[ xX]\](?: |$))?/;
/** Text before the cursor when typing `]` should turn the line into a task: `[`, `[ `, `- [`. */
const SHORTCUT_RE = /^(\s*)(?:[-*+] )?\[ ?$/;

const TASK_PREFIX = '- [ ] ';

class CheckboxWidget extends WidgetType {
  constructor(readonly checked: boolean) {
    super();
  }

  eq(other: CheckboxWidget) {
    return other.checked === this.checked;
  }

  toDOM() {
    const box = document.createElement('input');
    box.type = 'checkbox';
    box.checked = this.checked;
    box.className = 'cm-task-checkbox';
    box.setAttribute('aria-label', this.checked ? 'Mark as not done' : 'Mark as done');
    return box;
  }

  ignoreEvent() {
    return false;
  }
}

const doneLine = Decoration.line({ class: 'cm-task-done' });

/** Position of the mark character between the brackets. */
function markPos(line: Line, m: RegExpExecArray) {
  return line.from + m[1].length + m[2].length + 2;
}

interface TaskDecorations {
  all: DecorationSet;
  /** Just the checkbox widgets, so the cursor and Backspace treat each as one unit. */
  boxes: DecorationSet;
}

function buildDecorations(view: EditorView): TaskDecorations {
  const all = new RangeSetBuilder<Decoration>();
  const boxes = new RangeSetBuilder<Decoration>();
  const { doc } = view.state;
  // Only the visible part of the document is scanned.
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = doc.lineAt(pos);
      const m = TASK_RE.exec(line.text);
      if (m) {
        const checked = m[3] !== ' ';
        const bracket = markPos(line, m) - 1;
        // Hide `- ` / `* ` / `+ ` along with the brackets; keep numbers visible.
        const start = /\d/.test(m[2]) ? bracket : line.from + m[1].length;
        const box = Decoration.replace({ widget: new CheckboxWidget(checked) });
        if (checked) all.add(line.from, line.from, doneLine);
        all.add(start, bracket + 3, box);
        boxes.add(start, bracket + 3, box);
      }
      pos = line.to + 1;
    }
  }
  return { all: all.finish(), boxes: boxes.finish() };
}

/** Flips the task on `line`. Returns false if it isn't a task line. */
function toggleLine(view: EditorView, line: Line): boolean {
  const m = TASK_RE.exec(line.text);
  if (!m) return false;
  const at = markPos(line, m);
  view.dispatch({
    changes: { from: at, to: at + 1, insert: m[3] === ' ' ? 'x' : ' ' },
    userEvent: 'input.toggle',
  });
  return true;
}

const taskPlugin = ViewPlugin.fromClass(
  class {
    decorations: TaskDecorations;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged) this.decorations = buildDecorations(u.view);
    }
  },
  {
    decorations: (v) => v.decorations.all,
    provide: (plugin) =>
      EditorView.atomicRanges.of((view) => view.plugin(plugin)?.decorations.boxes ?? Decoration.none),
    eventHandlers: {
      mousedown(e, view) {
        const target = e.target;
        if (target instanceof HTMLInputElement && target.classList.contains('cm-task-checkbox')) {
          e.preventDefault();
          return toggleLine(view, view.state.doc.lineAt(view.posAtDOM(target)));
        }
        return false;
      },
    },
  },
);

/** Typing `[]` at the start of a line makes it a task. */
const bracketShortcut = EditorView.inputHandler.of((view, from, to, text) => {
  if (text !== ']' || from !== to) return false;
  const line = view.state.doc.lineAt(from);
  const m = SHORTCUT_RE.exec(line.text.slice(0, from - line.from));
  if (!m) return false;
  const start = line.from + m[1].length;
  const insert = line.text[to - line.from] === ' ' ? TASK_PREFIX.trimEnd() : TASK_PREFIX;
  view.dispatch({
    changes: { from: start, to, insert },
    selection: { anchor: start + insert.length },
    userEvent: 'input.type',
  });
  return true;
});

/** Mod-Enter: toggle the task on the current line, or turn the line into a task. */
const toggleTask: Command = (view) => {
  const { state } = view;
  const line = state.doc.lineAt(state.selection.main.head);
  if (toggleLine(view, line)) return true;
  const item = ITEM_RE.exec(line.text);
  const insertAt = item ? line.from + item[0].length : line.from + /^\s*/.exec(line.text)![0].length;
  const changes = state.changes({ from: insertAt, insert: item ? '[ ] ' : TASK_PREFIX });
  view.dispatch({
    changes,
    // assoc 1: a cursor sitting where the prefix goes ends up after it.
    selection: state.selection.map(changes, 1),
    userEvent: 'input',
  });
  return true;
};

/**
 * Enter on a list/task item continues the list; Enter on an empty item
 * removes its prefix (like most todo apps).
 */
const continueList: Command = (view) => {
  const { state } = view;
  const sel = state.selection.main;
  if (!sel.empty) return false;
  const line = state.doc.lineAt(sel.head);
  const m = ITEM_RE.exec(line.text);
  if (!m || sel.head < line.from + m[0].length) return false;

  if (line.text.slice(m[0].length).trim() === '') {
    view.dispatch({
      changes: { from: line.from, to: line.to, insert: '' },
      userEvent: 'delete',
    });
    return true;
  }

  let bullet = m[2];
  if (/^\d/.test(bullet)) bullet = `${parseInt(bullet, 10) + 1}${bullet.slice(-1)}`;
  const prefix = `${m[1]}${bullet} ${m[3] ? '[ ] ' : ''}`;
  view.dispatch(state.replaceSelection(`\n${prefix}`), { scrollIntoView: true, userEvent: 'input' });
  return true;
};

export const taskKeymap: KeyBinding[] = [
  { key: 'Mod-Enter', run: toggleTask },
  { key: 'Enter', run: continueList },
];

export const tasks = [taskPlugin, bracketShortcut];
