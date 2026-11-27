-- Lifecycle permission is independent from permission to send agent prompts.
create table agent_threads.workspace_nodes (
    device_id uuid primary key references devices.devices(id) on delete cascade,
    ready boolean not null,
    seen_at timestamptz not null
);
create table agent_threads.workspace_owners (
    device_id uuid not null references devices.devices(id) on delete cascade,
    workspace uuid not null,
    principal uuid not null,
    created_at timestamptz not null,
    primary key (device_id, workspace)
);
create table agent_threads.workspace_requests (
    request uuid primary key,
    device_id uuid not null,
    workspace uuid not null,
    principal uuid not null,
    payload jsonb not null check (octet_length(payload::text) <= 4096),
    state text not null check (state in ('queued','offered','completed','unresolved','expired','revoked')),
    receipt jsonb check (octet_length(receipt::text) <= 8192),
    created_at timestamptz not null,
    expires_at timestamptz not null,
    foreign key (device_id, workspace) references agent_threads.workspace_owners(device_id, workspace) on delete cascade
);
create index workspace_requests_delivery on agent_threads.workspace_requests(device_id, created_at)
    where state in ('queued', 'offered');
