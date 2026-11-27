-- Private workspace preview identities are scoped to the person and one generation.
create table agent_threads.previews (
    endpoint uuid primary key,
    grant_id uuid not null unique,
    device_id uuid not null,
    workspace uuid not null,
    generation bigint not null check (generation between 1 and 9007199254740991),
    principal uuid not null,
    port integer not null check (port between 1 and 65535),
    created_at timestamptz not null,
    expires_at timestamptz not null check (expires_at > created_at and expires_at <= created_at + interval '10 minutes'),
    revoked boolean not null default false,
    foreign key (device_id, workspace) references agent_threads.workspaces(device_id, workspace) on delete cascade
);
create index managed_previews_workspace on agent_threads.previews(device_id, workspace, expires_at);
