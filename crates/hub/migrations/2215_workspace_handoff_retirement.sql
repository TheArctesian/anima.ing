alter table agent_threads.workspace_handoffs
    add column retire_requested_at timestamptz,
    add column retired_at timestamptz,
    add check (retire_requested_at is null or (
        finished_at is not null and activation_offered_at is not null
        and withdraw_requested_at is null
    )),
    add check (retired_at is null or retire_requested_at is not null);

create index workspace_handoffs_retirement on agent_threads.workspace_handoffs(source_device, retire_requested_at)
    where retire_requested_at is not null and retired_at is null;
