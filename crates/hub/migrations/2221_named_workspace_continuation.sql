alter table agent_threads.workspace_move_queue
    add column destination_device uuid
    check (destination_device <> source_device);
