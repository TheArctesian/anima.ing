-- K10 6.5. The launch is the device-pinned K10 item. The original claim remains
-- available after attempt finality. A launch ID is scoped to its account.
create table agent_threads.start_offers (
    device_id uuid primary key references devices.devices (id) on delete cascade,
    account_id uuid not null,
    pairs jsonb not null check (jsonb_typeof(pairs) = 'array'),
    enabled boolean not null,
    expires_at timestamptz not null
);

create table agent_threads.fingerprint_keys (
    version smallint primary key,
    key bytea not null check (octet_length(key) = 32),
    records integer not null default 0 check (records between 0 and 100000),
    receipt_bytes bigint not null default 0 check (receipt_bytes >= 0),
    receipt_byte_cap bigint not null default 67108864 check (receipt_byte_cap > 0)
);

insert into agent_threads.fingerprint_keys (version, key)
values (1, uuid_send(gen_random_uuid()) || uuid_send(gen_random_uuid()));

create table agent_threads.launches (
    account_id uuid not null,
    launch_id uuid not null,
    work_id uuid not null unique,
    device_id uuid not null,
    first_command uuid not null,
    fingerprint bytea not null check (octet_length(fingerprint) = 32),
    key_version smallint not null references agent_threads.fingerprint_keys (version),
    project text not null,
    folder text not null,
    program text not null check (program in ('claude_code', 'codex')),
    access text not null check (access in ('full_access', 'accept_edits', 'ask', 'plan')),
    admitted_at timestamptz not null,
    deadline timestamptz not null,
    claimed_at timestamptz,
    lease_end timestamptz,
    attempt_phase text check (attempt_phase in (
        'claimed', 'preparing', 'running', 'finishing', 'done', 'refused', 'failed', 'stopped', 'expired'
    )),
    runner_id uuid,
    authorization uuid,
    clock_generation uuid,
    issued_tick bigint,
    issued_at timestamptz,
    valid_until timestamptz,
    denial text check (denial in ('WORK_OFF', 'target_unavailable', 'LAUNCH_CONFLICT')),
    evidence text check (evidence in ('created', 'refused', 'failed', 'stopped', 'expired')),
    admission_cause text check (admission_cause in ('target_unavailable', 'expired')),
    publication text check (publication in ('timely', 'timed_out')),
    withdrawn boolean not null default false,
    control_closed boolean not null default false,
    device_removed_at timestamptz,
    thread_id uuid unique,
    retired boolean not null default false,
    primary key (account_id, launch_id),
    unique (account_id, device_id, launch_id),

    check ((authorization is null) = (valid_until is null)),
    check (authorization is null or
        (runner_id is not null and issued_at is not null and clock_generation is not null and issued_tick is not null)),
    check (evidence is distinct from 'created' or authorization is not null),
    check (thread_id is null or (evidence = 'created' and publication = 'timely'))
);

create unique index agent_thread_authorized_runner on agent_threads.launches (device_id, runner_id)
    where authorization is not null;

create index agent_thread_launch_queue on agent_threads.launches (device_id, admitted_at, work_id)
    where attempt_phase is null and evidence is null;
create index agent_thread_launch_active on agent_threads.launches (device_id, deadline)
    where not retired;

create table agent_threads.commands (
    account_id uuid not null,
    command_id text not null check (octet_length(command_id) between 1 and 256),
    device_id uuid not null,
    launch_id uuid not null,
    fingerprint bytea not null check (octet_length(fingerprint) = 32),
    key_version smallint not null references agent_threads.fingerprint_keys (version),
    kind text not null check (kind in ('prompt','answer','stop')),
    payload jsonb,
    state text not null check (state in ('waiting','delivered','recorded','refused','expired','retired')),
    cause text,
    created_at timestamptz not null,
    deadline timestamptz not null,
    delivered_at timestamptz,
    closed_at timestamptz,
    primary key (account_id, command_id),
    foreign key (account_id, device_id, launch_id)
        references agent_threads.launches (account_id, device_id, launch_id),
    check (state <> 'waiting' or payload is not null),
    check (state in ('waiting','delivered') or payload is null),
    check (state <> 'delivered' or delivered_at is not null),
    check ((state in ('recorded','refused','expired','retired')) = (closed_at is not null))
);

create index agent_thread_command_queue on agent_threads.commands (device_id, created_at, command_id)
    where state in ('waiting','delivered');

create index agent_thread_command_retirement on agent_threads.commands (closed_at)
    where state in ('recorded','refused','expired');

create table agent_threads.control_offers (
    account_id uuid not null,
    launch_id uuid not null,
    device_id uuid not null references devices.devices (id) on delete cascade,
    revision bigint not null check (revision between 1 and 9007199254740991),
    offer jsonb,
    fingerprint bytea not null check (octet_length(fingerprint) = 32),
    key_version smallint not null references agent_threads.fingerprint_keys (version),
    expires_at timestamptz not null,
    primary key (account_id, launch_id),
    foreign key (account_id, device_id, launch_id)
        references agent_threads.launches (account_id, device_id, launch_id)
);

-- Receipt identity and original evidence outlive the device. Private command or
-- control content does not. The tombstone excludes any later reuse of the ID.
create function agent_threads.scrub_device() returns trigger
language plpgsql set search_path = pg_catalog as $$
begin
    if tg_op = 'UPDATE' then
        delete from agent_threads.start_offers where device_id = old.id;
        update agent_threads.control_offers set offer = null, expires_at = clock_timestamp()
        where device_id = old.id;
        return new;
    end if;

    update agent_threads.launches
    set device_removed_at = coalesce(device_removed_at, clock_timestamp()),
        thread_id = null, folder = '', project = ''
    where device_id = old.id;

    update agent_threads.commands
    set payload = null,
        state = case when state = 'waiting' then 'expired' else state end,
        closed_at = case when state = 'waiting' then clock_timestamp() else closed_at end
    where device_id = old.id and state in ('waiting', 'delivered') and payload is not null;
    return old;
end;
$$;

create trigger agent_thread_device_removed
after delete on devices.devices
for each row execute function agent_threads.scrub_device();

create trigger agent_thread_device_source_changed
after update of metrics_source on devices.devices
for each row when (old.metrics_source = 'daemon' and new.metrics_source <> 'daemon')
execute function agent_threads.scrub_device();
