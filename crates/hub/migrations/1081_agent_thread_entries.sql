create table agent_threads.entries (
    thread_id uuid not null references agent_threads.summaries (id) on delete cascade,
    sequence bigint not null check (sequence between 1 and 9007199254740991),
    content jsonb not null check (jsonb_typeof(content) = 'object'),
    primary key (thread_id, sequence)
);

create index summaries_activity_idx on agent_threads.summaries (activity_at desc, id desc);
