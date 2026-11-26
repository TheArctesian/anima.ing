-- Look up observed applications without scanning the device's job history.
create index jobs_application_applied on jobs.jobs (machine_id, (parameters->>'application'))
    where kind = 'host' and work_type = 'application_apply' and state = 'done';
