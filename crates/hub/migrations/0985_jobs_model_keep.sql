-- The work type `model_keep`: the idle time and the pin of a copy on one node (K2 range 0980-0999;
-- compute plan task L1-16; contract K11 0.7 Sections 2, 3 and 11.1; FIT-022, FIT-023).
--
-- Privileges: unchanged.

alter table jobs.jobs drop constraint jobs_work_type_check;
alter table jobs.jobs add constraint jobs_work_type_check check (work_type in (
    'build', 'runner',
    'model_report', 'runtime_install', 'model_pull', 'copy_delete', 'copy_adopt', 'server_start',
    'server_stop', 'model_load', 'model_unload', 'model_measure', 'model_keep', 'model_drain'
));
