/**
 * Composer slash commands.
 *
 * A `/command` typed at the very start of the composer expands into a prompt
 * template. They are front-end sugar only: the expanded text is what gets
 * sent, so the transcript stays readable and the backend needs no new message
 * type. `{caret}` marks where the cursor lands after expansion — usually
 * straight after an `@`, so the mention menu takes over.
 *
 * Templates are translated, so the prompt is written in the language the user
 * is working in; this module only handles the mechanics.
 */

export interface SlashCommand {
  name: string;
}

/** Stable names the user types. Hints and templates live in `slash.<name>.*`. */
export const SLASH_COMMANDS: SlashCommand[] = [
  { name: 'summarize' },
  { name: 'timeline' },
  { name: 'deadlines' },
  { name: 'compare' },
  { name: 'draft' },
];

/**
 * The `/` query being typed, or null. Only a command at position 0 counts, and
 * only while the caret is still inside it — `and/or` mid-sentence is prose.
 */
export function slashFragment(text: string, caret: number): string | null {
  const m = /^\/([a-z]*)$/i.exec(text.slice(0, caret));
  return m ? m[1].toLowerCase() : null;
}

export function matchSlash(query: string): SlashCommand[] {
  const q = query.toLowerCase();
  return SLASH_COMMANDS.filter((c) => c.name.startsWith(q));
}

/**
 * Replaces the `/query` at the start of `text` (up to `caret`) with a
 * translated template. Returns the new text and where the caret goes.
 */
export function expandSlash(
  template: string,
  text: string,
  caret: number,
): { text: string; caret: number } {
  const at = template.indexOf('{caret}');
  const body = template.replace('{caret}', '');
  const rest = text.slice(caret);
  return { text: body + rest, caret: at < 0 ? body.length : at };
}
