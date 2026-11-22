<script lang="ts">
  import Band from '$lib/code/kit/Band.svelte';
  import IconButton from '$lib/code/kit/IconButton.svelte';
  import Kbd from '$lib/code/kit/Kbd.svelte';
  import Tooltip from '$lib/code/kit/Tooltip.svelte';

  let inbox = $state(true);
  let terminal = $state(true);
  let panel = $state(false);
</script>

<!-- The band over a chat, with the inbox toggle at its start and the panel toggles at its end (02 sections 4.2, 6). -->
<div class="frames">
  <div class="frame" data-kit="band-chat">
    <Band>
      {#snippet start()}
        <Tooltip text="Toggle main sidebar (⌘B)" side="below">
          <IconButton label="Toggle main sidebar" icon={inbox ? 'panel-left-close' : 'panel-left'} onclick={() => (inbox = !inbox)} />
        </Tooltip>
      {/snippet}
      <span class="crumb">anima<span class="slash">/</span><span class="title">Make the needs-you count match the list</span></span>
      {#snippet end()}
        <IconButton label="Toggle thread details" icon="square-menu" pressed={false} onclick={() => undefined} />
        <IconButton label="Toggle terminal drawer" icon="panel-bottom" pressed={terminal} onclick={() => (terminal = !terminal)} />
        <IconButton label="Toggle right panel" icon="panel-right" pressed={panel} onclick={() => (panel = !panel)} />
        <IconButton label="Maximize panel" icon="maximize-2" disabled onclick={() => undefined} />
      {/snippet}
    </Band>
  </div>
  <div class="frame" data-kit="band-empty">
    <Band ruled>
      <span class="title">No active thread</span>
      {#snippet end()}<Kbd keys="⌘N" />{/snippet}
    </Band>
  </div>
</div>

<style>
  .frames {
    display: grid;
    gap: var(--space-4);
  }

  .frame {
    overflow: hidden;
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-lg);
  }

  .crumb {
    display: flex;
    gap: var(--space-2);
    min-inline-size: 0;
    color: var(--code-quiet-text);
  }

  .slash {
    color: var(--code-quiet-text);
  }

  .title {
    overflow: hidden;
    color: var(--text);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
