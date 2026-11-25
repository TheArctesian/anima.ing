-- K8 4.1 and 4.2. Leave legacy observations unknown.
alter table agent_threads.summaries add column first_published_at timestamptz;
