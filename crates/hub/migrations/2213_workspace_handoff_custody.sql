create table agent_threads.workspace_handoffs (
    transfer uuid primary key,
    workspace uuid not null,
    principal uuid not null,
    source_device uuid not null references devices.devices(id) on delete cascade,
    destination_device uuid not null references devices.devices(id) on delete cascade,
    generation bigint not null check (generation between 1 and 9007199254740990),
    storage_bytes bigint not null check (storage_bytes between 1073741824 and 68719476736),
    checkout jsonb check (octet_length(checkout::text) <= 1024),
    checkpoint jsonb check (octet_length(checkpoint::text) <= 4096),
    confirmed boolean not null default false check (not confirmed or checkpoint is not null),
    restored_at timestamptz check (restored_at is null or confirmed),
    activation_offered_at timestamptz check (activation_offered_at is null or restored_at is not null),
    created_at timestamptz not null,
    expires_at timestamptz not null,
    finished_at timestamptz,
    check (source_device <> destination_device)
);
create unique index workspace_handoffs_active on agent_threads.workspace_handoffs(workspace)
    where finished_at is null;
create index workspace_handoffs_source on agent_threads.workspace_handoffs(source_device, created_at)
    where finished_at is null;
create index workspace_handoffs_destination on agent_threads.workspace_handoffs(destination_device, created_at)
    where finished_at is null;
