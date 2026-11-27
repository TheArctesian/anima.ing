-- The work type `copy_adopt`: a machine uses a found model where it is (K2 range 0980-0999;
-- compute plan task L1-22; contract K11 0.6 Sections 2.3 and 3; PULL-012, R-444). Numbered 0987
-- at the merge with dev.
--
-- Privileges: unchanged.

alter table jobs.jobs drop constraint jobs_model_types;
alter table jobs.jobs add constraint jobs_model_types check (kind <> 'model' or work_type in (
    'model_report', 'runtime_install', 'model_pull', 'copy_delete', 'copy_adopt', 'server_start',
    'server_stop', 'model_load', 'model_unload', 'model_measure', 'model_drain'
));
