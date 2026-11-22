<script lang="ts">
  import { laidOut, type Attachment } from './attach';
  import FileList from './FileList.svelte';
  import MediaRow from './MediaRow.svelte';

  interface Props {
    attachments: readonly Attachment[];
    onRemove: (id: string) => void;
    onOpen: (id: string) => void;
    /** The ids of the files the text already shows as a chip: the list leaves them out. */
    chipped?: ReadonlySet<string>;
  }

  /* What is attached, over the editor (03 "Layout order in the composer body"): the media row, then the file list. */
  const { attachments, onRemove, onOpen, chipped }: Props = $props();
  const parts = $derived(laidOut(attachments, chipped));
</script>

{#if parts.media.length > 0}
  <MediaRow attachments={parts.media} {onRemove} {onOpen} />
{/if}
{#if parts.files.length > 0}
  <FileList attachments={parts.files} {onRemove} {onOpen} />
{/if}
