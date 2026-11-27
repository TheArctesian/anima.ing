-- The model items of the work route (K2 range 0980-0999; contract K11,
-- `contracts/compute/model-work.md`, accepted by R-344; task L1-5 of the compute plan). A model
-- item is a job of the kind `model`: it has no repository and no API key of a host item, and it
-- has the actor that asked for it (K11 Sections 2 and 3). No row holds a secret.
--
-- Numbered 0985 at the merge with dev, whose 0982 to 0984 (the host items of K10) took the
-- numbers first. The verdict `policy` of a decision is in 3600, after 2210 (OD-C30).

alter table jobs.jobs add column actor text check (actor in ('User', 'Agent', 'Hub'));

alter table jobs.jobs
    drop constraint jobs_work_shape,
    add constraint jobs_work_shape check (
        (work_type in ('build', 'runner') and kind in ('container', 'native')
            and forge is not null and owner is not null and name is not null
            and host_api_key is null and actor is null)
        or (kind = 'host' and forge is null and owner is null and name is null
            and machine_id is not null and project_id is null and attempt <= 1 and actor is null and (
                (work_type = 'container_action' and cardinality(secrets) = 0 and limit_seconds <= 120)
                or (work_type = 'application_apply' and
                    secrets in (array['compose_file'], array['compose_file', 'environment_file'])
                    and limit_seconds <= 1800)
            ))
        or (kind = 'model' and actor is not null
            and forge is null and owner is null and name is null and host_api_key is null)
    );

-- The work types of a model item (K11 Section 2). The types of the other kinds are in
-- `jobs_work_shape`.
alter table jobs.jobs add constraint jobs_model_types check (kind <> 'model' or work_type in (
    'model_report', 'runtime_install', 'model_pull', 'copy_delete', 'server_start',
    'server_stop', 'model_load', 'model_unload', 'model_measure', 'model_drain'
));
