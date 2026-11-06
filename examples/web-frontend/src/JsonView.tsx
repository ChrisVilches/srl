// JSON in colors: keys, strings, numbers, and true, false and null, each in
// its own color in both themes. Used for the JSON side of the card browser,
// and for ```json code blocks, such as a JSON note shown as context.

// A string (a key when a colon follows it), a number, or a literal.
const TOKEN = /("(?:[^"\\]|\\.)*")(\s*:)?|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\b(?:true|false|null)\b/g;

const COLORS = {
  key: 'text-sky-700 dark:text-sky-300',
  string: 'text-emerald-700 dark:text-emerald-300',
  number: 'text-amber-700 dark:text-amber-300',
  literal: 'text-violet-700 dark:text-violet-300',
};

const escape = (text: string) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

// TEXT as HTML, escaped, with its tokens in colored spans. Text that isn't
// JSON comes out escaped, with whatever looks like a token colored.
export function highlightJson(text: string): string {
  let html = '';
  let at = 0;
  for (const match of text.matchAll(TOKEN)) {
    const [token, string, colon] = match;
    html += escape(text.slice(at, match.index));
    if (string !== undefined) {
      const color = colon !== undefined ? COLORS.key : COLORS.string;
      html += `<span class="${color}">${escape(string)}</span>${colon ?? ''}`;
    } else {
      const color = /^[tfn]/.test(token) ? COLORS.literal : COLORS.number;
      html += `<span class="${color}">${token}</span>`;
    }
    at = match.index + token.length;
  }
  return html + escape(text.slice(at));
}

export function JsonView({ value }: { value: unknown }) {
  return (
    <pre
      className="overflow-x-auto rounded-2xl border border-stone-200 bg-white p-5 font-mono text-sm whitespace-pre-wrap text-stone-500 dark:border-stone-800 dark:bg-stone-900 dark:text-stone-400"
      dangerouslySetInnerHTML={{ __html: highlightJson(JSON.stringify(value, null, 2)) }}
    />
  );
}
