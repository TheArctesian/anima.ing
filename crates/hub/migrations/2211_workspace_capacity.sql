-- Capacity is optional for nodes using the original workspace poll.
alter table agent_threads.workspace_nodes
    add column capacity jsonb check (jsonb_typeof(capacity) = 'object' and octet_length(capacity::text) <= 256);
