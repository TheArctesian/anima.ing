-- R-270, R-186 and K10 Section 3. A project identifier is kept as a value,
-- like jobs.project_machines. A removed device immediately loses its grant.
create table jobs.project_devices (
    project_id uuid not null,
    device_id uuid not null references devices.devices (id) on delete cascade,
    primary key (project_id, device_id)
);
