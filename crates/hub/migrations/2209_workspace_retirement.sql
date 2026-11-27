alter table agent_threads.workspace_owners
    add column deleted boolean not null default false;

update agent_threads.workspace_owners w set deleted = true
where exists (
    select 1 from agent_threads.workspace_requests r
    where r.device_id = w.device_id and r.workspace = w.workspace
      and r.principal = w.principal and r.state = 'completed'
      and r.payload = r.receipt->'command'
      and r.payload->'action'->>'kind' = 'delete'
      and r.payload->'action'->'confirmed' = 'true'::jsonb
      and r.receipt->'result'->>'workspace' = w.workspace::text
      and r.receipt->'result'->>'phase' = 'deleted'
      and r.receipt->'result'->'storageBytes' = '0'::jsonb
      and r.receipt->'result'->'generation' = r.payload->'action'->'generation'
);
