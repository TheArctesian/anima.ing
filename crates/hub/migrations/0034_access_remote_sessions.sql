-- OD-E32: browser-approved remote sessions have no fixed or inactivity expiry.
alter table access.api_tokens
    alter column expires_at drop not null,
    drop constraint api_tokens_command_line_parent,
    add constraint api_tokens_command_line_parent check (
        parent_id is null or kind = 'command_line'
    ),
    add constraint api_tokens_expiry_required check (
        expires_at is not null or (kind = 'command_line' and parent_id is null)
    ),
    add constraint api_tokens_remote_no_idle check (
        kind <> 'command_line' or parent_id is not null or idle_seconds is null
    );

create table access.command_line_sign_ins (
    id uuid primary key,
    device_hash bytea not null unique check (octet_length(device_hash) = 32),
    user_hash bytea not null unique check (octet_length(user_hash) = 32),
    computer text not null check (char_length(computer) between 1 and 64),
    client text not null,
    created_at timestamptz not null,
    expires_at timestamptz not null,
    next_poll_at timestamptz not null,
    interval_seconds integer not null default 5 check (interval_seconds >= 5),
    state text not null default 'waiting' check (state in ('waiting', 'approved', 'denied', 'expired', 'consumed')),
    operator_id uuid references access.operators(id) on delete cascade
);
create index command_line_sign_ins_client on access.command_line_sign_ins(client, created_at);

create table access.command_line_code_attempts (
    operator_id uuid primary key references access.operators(id) on delete cascade,
    failures integer not null default 0,
    window_ends_at timestamptz not null
);
