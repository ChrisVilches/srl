import type { KeyboardEvent } from 'react';

// Help for writing cloze deletions in the question of a qa item, the
// built-in kind (docs/cards.md, "Cloze deletions"), and of no other kind:
// srl says nothing of a kind's syntax, so this is the page knowing qa, an
// exception kept to this file. The forms ask it for their field's key
// handler, and Add for a hint about it after the kind's description; any
// other kind or field gets none.
//
// It knows only how a deletion is written: it doesn't check one. srl does,
// when the item is checked, added or saved, and says what is wrong, such as
// an empty deletion or one inside another.

const isQuestion = (kind: string, field: string) => kind === 'qa' && field === 'Q';

// Ctrl+Shift+C (⌘+Shift+C), as in Anki.
const isShortcut = (event: KeyboardEvent) =>
  event.key.toLowerCase() === 'c' && event.shiftKey && (event.ctrlKey || event.metaKey) && !event.altKey;

// The number of the next deletion in QUESTION: one more than the highest
// one written, so that a new deletion never takes one already there.
export function nextCloze(question: string): number {
  const numbers = [...question.matchAll(/\{\{c(\d+)::/g)].map((match) => Number(match[1]));
  return Math.max(0, ...numbers) + 1;
}

// SELECTED, text of QUESTION, as the next deletion of QUESTION.
export const deletion = (question: string, selected: string) => `{{c${nextCloze(question)}::${selected}}}`;

// The key handler of the box of FIELD in a form for an item of KIND, or
// undefined unless it is qa's question.
export function clozeShortcut(kind: string, field: string) {
  if (!isQuestion(kind, field)) return undefined;
  return (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (!isShortcut(event)) return;
    event.preventDefault();
    const box = event.currentTarget;
    const { selectionStart: start, selectionEnd: end, value } = box;
    const made = deletion(value, value.slice(start, end));
    // Typed into the box in place of the selection, so that the form sees
    // it as any typing (an input event) and undo takes it back.
    document.execCommand('insertText', false, made);
    // With nothing selected, the cursor goes inside, to type its text.
    if (start === end) box.setSelectionRange(start + made.length - 2, start + made.length - 2);
  };
}

// The shortcut, as its key and what it does, said after the description of
// KIND, or undefined unless it is qa.
export const clozeHint = (kind: string) =>
  kind === 'qa'
    ? { key: 'ctrl+shift+c', text: 'makes the selected text of the question a cloze deletion.' }
    : undefined;
