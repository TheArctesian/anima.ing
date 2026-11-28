-- K11 Section 2. Model work carries a fixed actor and no repository.
alter table jobs.jobs
    add column actor text,
    drop constraint jobs_limit_memory_bytes_check,
    drop constraint jobs_limit_disk_bytes_check,
    add constraint jobs_limit_memory_bytes_check check (
        limit_memory_bytes > 0 or (kind = 'model' and limit_memory_bytes = 0)),
    add constraint jobs_limit_disk_bytes_check check (
        limit_disk_bytes > 0 or (kind = 'model' and limit_disk_bytes = 0)),
    drop constraint jobs_work_shape,
    add constraint jobs_work_shape check (
        (work_type in ('build', 'runner') and kind in ('container', 'native')
            and forge is not null and owner is not null and name is not null
            and host_api_key is null and actor is null)
        or (kind = 'host' and forge is null and owner is null and name is null
            and machine_id is not null and project_id is null and attempt <= 1
            and actor is null and (
                (work_type = 'container_action' and cardinality(secrets) = 0 and limit_seconds <= 120)
                or (work_type = 'application_apply' and
                    secrets in (array['compose_file'], array['compose_file', 'environment_file'])
                    and limit_seconds <= 1800)
            ))
        or (kind = 'model' and work_type in (
                'model_report', 'runtime_install', 'model_pull', 'copy_delete', 'server_start',
                'server_stop', 'model_load', 'model_unload', 'model_measure', 'model_drain')
            and actor is not null and actor in ('User', 'Agent', 'Hub')
            and project_id is null and forge is null and owner is null and name is null
            and host_api_key is null
            and (machine_id is not null or work_type in ('model_report', 'runtime_install'))
            and secrets = case
                when work_type = 'model_pull' and parameters->'gated' = 'true'::jsonb
                then array['hf_token']::text[] else array[]::text[]
            end)
    );

create index jobs_model_queue on jobs.jobs (
    (case work_type when 'model_drain' then 0 when 'server_stop' then 1
        when 'model_unload' then 2 else 3 end), created_at, id
) where kind = 'model' and state = 'queued';
