-- The model items of the work route (K2 range 0980-0999; contract K11,
-- `contracts/compute/model-work.md`, accepted by R-344; task L1-5 of the compute plan). A model
-- item is a job of the kind `model`: it has no repository, and it has the actor that asked for it
-- (K11 Sections 2 and 3). No row holds a secret.

alter table jobs.jobs drop constraint jobs_work_type_check;
alter table jobs.jobs add constraint jobs_work_type_check check (work_type in (
    'build', 'runner',
    'model_report', 'runtime_install', 'model_pull', 'copy_delete', 'server_start',
    'server_stop', 'model_load', 'model_unload', 'model_measure', 'model_drain'
));

alter table jobs.jobs drop constraint jobs_kind_check;
alter table jobs.jobs add constraint jobs_kind_check check (kind in ('container', 'native', 'model'));

alter table jobs.jobs
    alter column forge drop not null,
    alter column owner drop not null,
    alter column name drop not null,
    add column actor text check (actor in ('User', 'Agent', 'Hub'));

-- A model item has the kind `model`, a model type, an actor and no repository; each other job
-- has a repository, no actor and a type of K10.
alter table jobs.jobs add constraint jobs_model_check check (
    (kind = 'model') = (work_type not in ('build', 'runner'))
    and (kind = 'model') = (actor is not null)
    and (kind = 'model') = (forge is null)
    and (forge is null) = (owner is null)
    and (forge is null) = (name is null)
);

-- The device policy of the machine stops the model item now (CAP-010 to CAP-012).
alter table jobs.decisions drop constraint decisions_verdict_check;
alter table jobs.decisions add constraint decisions_verdict_check check (verdict in (
    'fits', 'type', 'kind', 'os', 'architecture', 'memory', 'disk', 'refused_before',
    'names_other', 'excluded', 'waits_for_better', 'policy'
));
