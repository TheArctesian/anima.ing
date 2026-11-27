alter table agent_threads.workspace_handoffs
    add column withdraw_requested_at timestamptz,
    add column destination_discarded_at timestamptz,
    add check (destination_discarded_at is null or (confirmed and withdraw_requested_at is not null));
