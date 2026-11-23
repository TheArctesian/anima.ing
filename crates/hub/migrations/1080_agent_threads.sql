-- TH20 and TH21: thread summaries from an enrolled device (K8 Section 4).
create schema agent_threads;

create table agent_threads.counts (
    device_id uuid primary key references devices.devices (id) on delete cascade,
    count bigint not null check (count between 0 and 4294967295),
    revision bigint not null check (revision between 1 and 9007199254740991)
);

create table agent_threads.summaries (
    id uuid primary key,
    device_id uuid not null references devices.devices (id) on delete cascade,
    runner_id uuid not null,
    program text not null check (program in ('claude_code', 'codex')),
    state text not null check (state in ('starting', 'working', 'needs_you', 'idle', 'stopped', 'unknown')),
    title text not null check (char_length(title) <= 80),
    started_at timestamptz not null,
    activity_at timestamptz not null check (activity_at >= started_at),
    repository text,
    revision bigint not null check (revision between 1 and 9007199254740991),
    unique (device_id, runner_id)
);
