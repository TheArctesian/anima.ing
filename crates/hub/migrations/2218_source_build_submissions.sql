create table jobs.source_build_submissions (
    request uuid primary key,
    operator uuid not null,
    repository uuid not null,
    job uuid not null unique references jobs.jobs(id) on delete cascade
);
