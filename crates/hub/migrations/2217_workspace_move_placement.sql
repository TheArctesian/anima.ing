create table agent_threads.workspace_move_queue (
    transfer uuid primary key,
    workspace uuid not null,
    principal uuid not null,
    source_device uuid not null references devices.devices(id) on delete cascade,
    generation bigint not null check (generation between 1 and 9007199254740990),
    storage_bytes bigint not null check (storage_bytes between 1073741824 and 68719476736),
    continue_session boolean not null,
    limits jsonb not null check (octet_length(limits::text) <= 1024),
    state text not null default 'waiting' check (state in ('waiting', 'placed', 'withdrawn', 'expired')),
    created_at timestamptz not null,
    expires_at timestamptz not null,
    checked_at timestamptz
);

create unique index workspace_move_one_waiting
    on agent_threads.workspace_move_queue(workspace) where state='waiting';
create index workspace_move_placement_order
    on agent_threads.workspace_move_queue(checked_at nulls first, created_at, transfer) where state='waiting';

create index workspace_move_placement_expiry
    on agent_threads.workspace_move_queue(expires_at) where state='waiting';

create function agent_threads.workspace_destination_available(node uuid, project uuid, actor uuid, moving_generation bigint)
returns boolean language sql stable as $$
    select not exists (
        select 1 from agent_threads.workspace_owners w
        where w.device_id=node and w.workspace=project and not (
            w.deleted and w.principal=actor and coalesce((
                select h.retired_at is not null and h.generation<moving_generation
                from agent_threads.workspace_handoffs h
                where h.source_device=w.device_id and h.workspace=w.workspace and h.principal=w.principal
                    and h.created_at>=w.created_at and h.finished_at is not null
                    and h.activation_offered_at is not null and h.withdraw_requested_at is null
                order by h.finished_at desc,h.created_at desc,h.transfer desc limit 1
            ),false)
        )
    );
$$;

create function jobs.workspace_placement_allowed(node uuid, observed_at timestamptz)
returns boolean language sql stable as $$
    select not exists (
        select 1 from jobs.machines m where m.device_id=node and
        (m.switch='off' or m.unclean or m.excluded or m.off_by_operator or m.off_until>observed_at)
    );
$$;

create function agent_threads.workspace_placement_available(node uuid, observed_at timestamptz)
returns boolean language sql stable as $$
    select jobs.workspace_placement_allowed(node,observed_at) and not exists (
        select 1 from agent_threads.workspace_handoffs h
        where h.destination_device=node and h.finished_at is null
    );
$$;
