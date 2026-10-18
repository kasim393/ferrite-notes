// Loaded lazily so the editor opens before the markdown parser arrives.
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { tags } from '@lezer/highlight';

const style = HighlightStyle.define([
  { tag: tags.heading1, fontWeight: '700', fontSize: '1.3em' },
  { tag: tags.heading2, fontWeight: '700', fontSize: '1.15em' },
  { tag: [tags.heading3, tags.heading4, tags.heading5, tags.heading6], fontWeight: '700' },
  { tag: tags.strong, fontWeight: '700' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.strikethrough, textDecoration: 'line-through' },
  { tag: [tags.link, tags.url], color: 'var(--accent)' },
  { tag: tags.monospace, fontFamily: 'var(--font-mono)' },
  { tag: [tags.processingInstruction, tags.meta, tags.quote], color: 'var(--text-muted)' },
]);

export const markdownSupport = [markdown({ base: markdownLanguage }), syntaxHighlighting(style)];
