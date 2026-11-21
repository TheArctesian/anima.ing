<script lang="ts">
  import ApprovalRow from '$lib/code/approval/ApprovalRow.svelte';

  let answer = $state('none');
  const decide = (decision: string): void => {
    answer = decision;
  };
  const LONG = Array.from({ length: 14 }, (_, index) => `sed -i '' 's/old-${String(index)}/new-${String(index)}/g' src/lib/part-${String(index)}.ts`).join(' &&\n');
</script>

<div class="lane">
  <div data-kit="approval-command"><ApprovalRow kind="command" detail="git status --short" onDecide={decide} /></div>
  <div data-kit="approval-file-change"><ApprovalRow kind="file-change" detail="apps/client/src/lib/code/kit/Menu.svelte" onDecide={decide} /></div>
  <div data-kit="approval-file-read"><ApprovalRow kind="file-read" detail="~/.ssh/config" reason="To find the host alias of the build machine." onDecide={decide} /></div>
  <div data-kit="approval-permission"><ApprovalRow kind="permission" detail="Run commands with network access" app="Codex" reason="The test downloads its fixtures." onDecide={decide} /></div>
  <div data-kit="approval-app-access"><ApprovalRow kind="app-access" detail="anima mcp: list_devices" app="Claude Code" onDecide={decide} /></div>
  <div data-kit="approval-long"><ApprovalRow kind="command" detail={LONG} onDecide={decide} /></div>
  <div data-kit="approval-locked"><ApprovalRow kind="command" detail="bun run gates:changed" locked onDecide={decide} /></div>
  <div data-kit="approval-gone"><ApprovalRow kind="command" detail="cargo nextest run --workspace" gone onDecide={decide} /></div>
  <p class="answer" aria-live="polite">Answer: {answer}</p>
</div>

<style>
  .lane {
    display: grid;
    gap: var(--space-6);
    max-inline-size: var(--code-lane);
  }

  .answer {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-xs);
  }
</style>
