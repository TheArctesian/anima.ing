-- One-use POST handoffs become host-only browser sessions; raw tokens are never stored.
create table agent_threads.preview_sessions (
    handoff_hash bytea primary key check (octet_length(handoff_hash)=32),
    session_hash bytea unique check (octet_length(session_hash)=32),
    endpoint uuid not null references agent_threads.previews(endpoint) on delete cascade,
    principal uuid not null,
    handoff_expires timestamptz not null,
    expires_at timestamptz not null,
    redeemed boolean not null default false
);
create index preview_sessions_endpoint on agent_threads.preview_sessions(endpoint);
