alter table agent_threads.workspace_handoffs
    add column continue_session boolean not null default false;

create table agent_threads.workspace_continuations (
    transfer uuid primary key references agent_threads.workspace_handoffs(transfer) on delete cascade,
    previous_command uuid not null,
    provider text not null check (provider in ('codex','claude')),
    workspace_request uuid unique,
    agent_request uuid not null unique,
    workspace_action jsonb not null check (octet_length(workspace_action::text) <= 1024),
    provider_ready_at timestamptz
);

create index workspace_handoffs_activated_destination
    on agent_threads.workspace_handoffs(destination_device, finished_at, transfer)
    where finished_at is not null and activation_offered_at is not null and withdraw_requested_at is null;
