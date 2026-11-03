// How to write a selector (docs/selectors.md), shown under every error srl
// gives for a selector, whatever the error.

// Whether srl's message is about a selector: srl starts every such error
// with these words.
export function isSelectorError(message: string): boolean {
  return message.includes('invalid selector term');
}

const RULES: [string, string][] = [
  ['in:a in:b', 'A space means and: cards in a and in b.'],
  ['in:a,b', 'A comma means or, between the values of one key, with no space around it: cards in a or in b.'],
  ['-in:a', 'A minus means not: cards that are not in a.'],
  ['"some words"', 'Double quotes make one term of text, spaces and colons included.'],
];

const EXAMPLES: [string, string][] = [
  ['in:grammar', 'Cards whose address has a part named grammar, its extension aside, such as japanese/grammar/ or grammar.md'],
  ['in:grammar,kanji', 'Cards in grammar or in kanji'],
  ['in:japanese -in:kanji', 'Cards in japanese, but not in kanji'],
  ['path:japanese/*', 'Cards of the collections directly in japanese/; path:japanese/** for everything below it'],
  ['is:due', 'Cards due today; a card is new, due, young or mature in its schedule'],
  ['is:new in:grammar', 'New cards in grammar'],
  ['is:draft', 'Draft cards; a card is active, draft or suspended'],
  ['is:suspended', 'Suspended cards: in Browse, the cards to fix, each with why'],
  ['is:leech', 'Cards forgotten again and again, whose question likely needs rewriting'],
  ['kind:qa', 'Cards of items of a kind: qa, or a kind of the store'],
  ['source:notes', 'Cards whose items come from a source of the store'],
  ['id:k3m9q1', 'The cards of an item, or a card, by its ID'],
  ['verb', 'Cards whose item contains the text, in any field, ignoring case'],
  ['"std::move"', 'Text with a colon or a space, in double quotes'],
];

function Rows({ rows }: { rows: [string, string][] }) {
  return (
    <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1.5">
      {rows.map(([selector, meaning]) => (
        <div key={selector} className="contents">
          <dt>
            <code className="font-mono text-stone-800 dark:text-stone-200">{selector}</code>
          </dt>
          <dd>{meaning}</dd>
        </div>
      ))}
    </dl>
  );
}

export function SelectorHelp() {
  return (
    <section className="flex flex-col gap-3 rounded-lg border border-stone-200 p-4 text-sm text-stone-600 dark:border-stone-800 dark:text-stone-300">
      <h2 className="font-semibold text-stone-800 dark:text-stone-200">Writing a selector</h2>
      <p>
        A selector is a list of terms separated by spaces. A term is a <code className="font-mono">key:value</code>{' '}
        or text. The keys are <code className="font-mono">in</code>, <code className="font-mono">path</code>,{' '}
        <code className="font-mono">source</code>, <code className="font-mono">id</code>,{' '}
        <code className="font-mono">kind</code> and <code className="font-mono">is</code>; any other key is an
        error. An empty selector is every card.
      </p>
      <Rows rows={RULES} />
      <p>There is no grouping: for something these rules can't say, search twice.</p>
      <h3 className="font-semibold text-stone-800 dark:text-stone-200">Examples</h3>
      <Rows rows={EXAMPLES} />
    </section>
  );
}
