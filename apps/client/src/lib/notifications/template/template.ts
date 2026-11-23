// D-21: substitution only, and only the hub's placeholders; the hub refuses any other `{name}` too (VALIDATION).
import type { NotificationTemplateInput } from '../api';

/** The D-21 placeholders, used only until the hub's own list (`NotificationSettings.placeholders`) has arrived. */
export const D21_PLACEHOLDERS: readonly string[] = ['{device}', '{check}', '{status}', '{previous}', '{message}', '{at}', '{days}'];

const NAMED_MAX = 3;
const SHOWN_MAX = 32;

export const TITLE_MISSING = 'Enter a title.';
export const BODY_MISSING = 'Enter a body.';

function unknownPlaceholders(text: string, allowed: readonly string[]): string[] {
  const found = [...text.matchAll(/\{[^{}]*\}/g)].map((match) => match[0]);
  return [...new Set(found)].filter((placeholder) => !allowed.includes(placeholder));
}

function shown(placeholder: string): string {
  return [...placeholder].length > SHOWN_MAX ? `${[...placeholder].slice(0, SHOWN_MAX - 1).join('')}…` : placeholder;
}

function lineProblem(text: string, allowed: readonly string[], missing: string): string {
  if (text.trim() === '') return missing;
  const unknown = unknownPlaceholders(text, allowed);
  if (unknown.length === 0) return '';
  const named = unknown.slice(0, NAMED_MAX).map(shown).join(', ');
  const more = unknown.length > NAMED_MAX ? ' and others' : '';
  return `Not a placeholder: ${named}${more}.`;
}

export interface TemplateProblems {
  readonly title: string;
  readonly body: string;
}

export function templateProblems(template: NotificationTemplateInput, allowed: readonly string[]): TemplateProblems {
  return { title: lineProblem(template.title, allowed, TITLE_MISSING), body: lineProblem(template.body, allowed, BODY_MISSING) };
}

export function templateFine(problems: TemplateProblems): boolean {
  return problems.title === '' && problems.body === '';
}
