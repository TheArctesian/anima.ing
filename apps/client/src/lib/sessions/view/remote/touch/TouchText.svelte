<script lang="ts">
  import type { SessionTunnel } from '../../../tunnel/guacamole/tunnel';

  const { client }: { client: Pick<SessionTunnel['client'], 'sendKeyEvent'> } = $props();
  const BACKSPACE = 0xff08;
  const ENTER = 0xff0d;
  const UNICODE = 0x01000000;

  function key(value: number): void {
    client.sendKeyEvent(1, value);
    client.sendKeyEvent(0, value);
  }

  function entered(event: Event): void {
    if (event instanceof InputEvent && event.isComposing) return;
    const field = event.currentTarget;
    if (!(field instanceof HTMLInputElement)) return;
    const text = field.value;
    field.value = '';
    for (const character of text) {
      const point = character.codePointAt(0);
      if (point === undefined || point < 0x20 || (point >= 0x7f && point < 0xa0)) continue;
      key(point < 0x100 ? point : UNICODE + point);
    }
  }

  function edit(event: InputEvent): void {
    if (event.inputType === 'insertFromPaste' || event.inputType === 'insertFromDrop') {
      event.preventDefault();
      return;
    }
    if (event.inputType === 'deleteContentBackward' || event.inputType === 'insertLineBreak') {
      event.preventDefault();
      key(event.inputType === 'deleteContentBackward' ? BACKSPACE : ENTER);
    }
  }

  function control(event: KeyboardEvent): void {
    if (event.isComposing || (event.key !== 'Enter' && event.key !== 'Backspace')) return;
    event.preventDefault();
    key(event.key === 'Enter' ? ENTER : BACKSPACE);
  }
</script>

<input
  type="text"
  aria-label="Type on remote computer"
  placeholder="Type"
  autocomplete="off"
  autocapitalize="none"
  spellcheck="false"
  onbeforeinput={edit}
  oninput={entered}
  onkeydown={control}
  onpaste={(event) => event.preventDefault()}
  ondrop={(event) => event.preventDefault()}
/>

<style>
  /* A field of the bar, not a row of its own. iOS enlarges the page on focus when a field's text is under 16 px. */
  input {
    box-sizing: border-box;
    flex: 1;
    min-inline-size: 0;
    block-size: var(--control-md);
    padding-inline: var(--space-2);
    font: inherit;
    font-size: var(--text-lg);
    color: var(--text);
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-md);
    background: var(--surface-base);
  }
</style>
