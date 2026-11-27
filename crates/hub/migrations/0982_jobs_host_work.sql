alter table jobs.jobs
    drop constraint jobs_work_type_check,
    drop constraint jobs_kind_check,
    alter column forge drop not null,
    alter column owner drop not null,
    alter column name drop not null,
    add column host_api_key text check (char_length(host_api_key) between 1 and 64),
    add constraint jobs_work_shape check (
        (work_type in ('build', 'runner') and kind in ('container', 'native')
            and forge is not null and owner is not null and name is not null
            and host_api_key is null)
        or (work_type = 'container_action' and kind = 'host'
            and forge is null and owner is null and name is null
            and machine_id is not null and project_id is null and cardinality(secrets) = 0
            and limit_seconds <= 120 and attempt <= 1)
    );

create index jobs_host_queue on jobs.jobs (
    machine_id, (parameters->>'action' = 'stop') desc, (host_api_key is not null), created_at, id
) where kind = 'host' and state = 'queued';

create index jobs_host_key on jobs.jobs (machine_id, host_api_key, created_at)
    where host_api_key is not null;
