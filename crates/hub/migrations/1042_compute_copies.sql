-- The copies of the model variants on the machines (K2 range 1040-1059; compute plan task L1-7;
-- contract K11 Sections 7 and 11; PULL-001, PULL-007, PULL-009, CMP-006).
--
-- Each variant gets an identifier, the `variant` of a `model_pull` (K11 Section 2). A re-read of
-- the library replaces a variant and gives it a new identifier.
--
-- `copies` holds one copy of a variant on one machine: what the pull took (the model, the tag,
-- the repository at its commit, the size and the count of files), so that the record stays true
-- when the library changes, and the job of its pull and of its removal. The state of a copy
-- comes from those jobs (`jobs.jobs`, migration 0983 for the progress): the hub keeps no second
-- copy of it. A machine has one copy of each `model:tag`. Removing a device removes its copies.
--
-- Privileges: migration 0001's default privileges give `anima_runtime` SELECT, INSERT, UPDATE
-- and DELETE on the new table. No row holds a secret.

alter table compute.variants add column id uuid not null default gen_random_uuid();
alter table compute.variants add constraint variants_id_key unique (id);

create table compute.copies (
    id uuid not null,
    device_id uuid not null,
    variant_id uuid not null,
    model text not null,
    tag text not null,
    format text not null,
    quantisation text not null,
    repository text not null,
    revision text not null,
    size_bytes bigint not null,
    files integer not null,
    pull_job_id uuid not null,
    delete_job_id uuid null,
    created_at timestamptz not null,
    constraint copies_pkey primary key (id),
    constraint copies_device_id_fkey foreign key (device_id)
        references devices.devices (id) on delete cascade,
    constraint copies_device_model_tag_key unique (device_id, model, tag),
    constraint copies_format_check check (format in ('gguf', 'mlx', 'safetensors')),
    constraint copies_size_bytes_check check (size_bytes > 0),
    constraint copies_files_check check (files between 1 and 64)
);
