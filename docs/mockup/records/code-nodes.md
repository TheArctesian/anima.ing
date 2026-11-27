# Code · Nodes mockup record

- Mockup: [code-nodes/index.html](../code-nodes/index.html)
- Owner: [AnimaOS redesign](../../proposals/spare-computer-os.md) and [managed Code delivery](../../../contracts/managed-code.md)
- Screen: [NodeCode.svelte](../../../apps/client/src/lib/code/nodes/NodeCode.svelte)
- State: Source implementation, 2026-11-24. Rendering and acceptance pending; local and VM checks are authorized.
- Scope: Node/workspace selection, enablement, provider/permission selection, composer, delivery receipt and uncertainty states. Workspace lifecycle adds storage preparation, bounded startup, explicit stop and permanent-deletion confirmations, reconciliation and persistent request recovery. The capacity example keeps the node connected and deletion available while preparation and start are held. Codex sign-in adds a transient verification challenge; Claude adds a hidden returned-code field and one-time submission. Both have explicit cancellation and completion/failure states, and changing the selected provider ends the old sign-in. The move example shows automatic destination choice, the unchecked continuation choice, the destination sign-in message and the selected preview waiting for its destination server. Narrow screens stack the picker above the conversation lane.
- Limits: The static reference shows delivery receipts, not an agent transcript. Managed transcript/approval and workspace lifecycle controls exist in source; their integrated behavior remains unqualified. No browser, desktop or mobile visual comparison has been run.
