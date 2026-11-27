-- Managed Code: enrolled nodes advertise; only the person enables delivery.
create table agent_threads.workspaces (
    device_id uuid not null references devices.devices(id) on delete cascade,
    workspace uuid not null,
    generation bigint not null check (generation between 1 and 9007199254740991),
    ready boolean not null default false,
    enabled boolean not null default false,
    seen_at timestamptz not null,
    primary key (device_id, workspace)
);
create table agent_threads.commands (
    request uuid primary key,
    device_id uuid not null,
    workspace uuid not null,
    generation bigint not null,
    principal uuid not null,
    payload jsonb not null check (octet_length(payload::text) <= 65536),
    state text not null check (state in ('queued','offered','completed','unresolved','revoked','expired')),
    result jsonb check (octet_length(result::text) <= 1100000),
    created_at timestamptz not null,
    expires_at timestamptz not null,
    foreign key (device_id, workspace) references agent_threads.workspaces(device_id, workspace) on delete cascade
);
create index managed_commands_delivery on agent_threads.commands(device_id, created_at) where state in ('queued','offered');
