-- K10 Sections 6.4.6 and 8. A managed application is bound to one machine.
create table jobs.application_machines (
    application_id uuid primary key,
    device_id uuid not null
);

alter table jobs.jobs
    drop constraint jobs_work_shape,
    add constraint jobs_work_shape check (
        (work_type in ('build', 'runner') and kind in ('container', 'native')
            and forge is not null and owner is not null and name is not null
            and host_api_key is null)
        or (kind = 'host' and forge is null and owner is null and name is null
            and machine_id is not null and project_id is null and attempt <= 1 and (
                (work_type = 'container_action' and cardinality(secrets) = 0 and limit_seconds <= 120)
                or (work_type = 'application_apply' and
                    secrets in (array['compose_file'], array['compose_file', 'environment_file'])
                    and limit_seconds <= 1800)
            ))
    );

create table jobs.application_files (
    job_id uuid not null references jobs.jobs (id) on delete cascade,
    purpose text not null check (purpose in ('compose_file', 'environment_file')),
    sealed bytea not null check (
        octet_length(sealed) between 1 and
        case when purpose = 'compose_file' then 262656 else 66048 end
    ),
    primary key (job_id, purpose)
);

drop index jobs.jobs_host_queue;
create index jobs_host_queue on jobs.jobs (
    machine_id, ((parameters->>'action' = 'stop') is true) desc,
    (host_api_key is not null), created_at, id
) where kind = 'host' and state = 'queued';

create index jobs_application_active on jobs.jobs (machine_id, (parameters->>'application'))
    where work_type = 'application_apply' and state in ('claimed', 'preparing', 'running', 'finishing');
