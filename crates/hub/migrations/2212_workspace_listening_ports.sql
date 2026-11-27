-- TCP listener observations are published only with the matching workspace inventory.
alter table agent_threads.workspaces
    add column listening_ports integer[] not null default '{}'
    check (cardinality(listening_ports) <= 16 and 1 <= all(listening_ports) and 65535 >= all(listening_ports));
