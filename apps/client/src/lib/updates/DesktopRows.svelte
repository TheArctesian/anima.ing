<script lang="ts">
  import BuildsRow from '$lib/builds/BuildsRow.svelte';
  import { capabilities } from '$lib/platform/platform';
  import LogFolderRow from '$lib/process-log/LogFolderRow.svelte';
  import CommandLineRow from './CommandLineRow.svelte';
  import UpdatesRow from './UpdatesRow.svelte';

  const { updates, commandLine, processLog, builds } = capabilities();
</script>

{#if updates}
  <UpdatesRow />
{/if}
{#if commandLine}
  <CommandLineRow />
{/if}
{#if processLog}
  <LogFolderRow />
{/if}
{#if builds}
  <BuildsRow />
{/if}
