import { flushSync } from 'svelte';

export interface Choice {
  readonly options: readonly HTMLButtonElement[];
  readonly value: string;
  pick(value: string): void;
}

export function choiceOf(target: HTMLElement, label: string): Choice {
  const group = target.querySelector<HTMLElement>(`[role="radiogroup"][aria-label="${label}"]`);
  if (group === null) throw new Error(`no ${label}`);
  const radios = (): HTMLButtonElement[] => [...group.querySelectorAll<HTMLButtonElement>('[role="radio"]')];
  return {
    get options() {
      return radios();
    },
    get value() {
      return radios().find((radio) => radio.getAttribute('aria-checked') === 'true')?.dataset['value'] ?? '';
    },
    pick(value: string) {
      radios()
        .find((radio) => radio.dataset['value'] === value)
        ?.click();
      flushSync();
    },
  };
}
