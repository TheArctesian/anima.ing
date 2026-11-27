alter table jobs.source_build_submissions
    add column checkout_account uuid,
    add column checkout_repository text check (checkout_repository ~ '^[1-9][0-9]{0,19}$'),
    add check ((checkout_account is null) = (checkout_repository is null));

create table jobs.repository_token_releases (
    job uuid not null references jobs.jobs(id) on delete cascade,
    attempt integer not null check (attempt > 0),
    device uuid not null,
    sealed bytea,
    expires_at timestamptz,
    released_at timestamptz,
    retry_at timestamptz,
    primary key (job, attempt),
    check (sealed is null or expires_at is not null)
);

create index repository_token_cleanup on jobs.repository_token_releases(expires_at, job, attempt)
    where sealed is not null;
