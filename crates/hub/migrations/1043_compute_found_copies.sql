-- Found copies: a model that a different program keeps on the machine, used where it is
-- (K2 range 1040-1059; compute plan task L1-22; contract K11 Section 2.3; PULL-012, R-444).
--
-- A found copy has no variant of the library: `variant_id` is null, `found` is the key that
-- the daemon gave the model in its `model_report`, `model` is the name that the tool shows,
-- `tag`, `quantisation` and `revision` are empty and `repository` is the tool word.
-- `pull_job_id` is the job that brought the copy: `model_pull`, or `copy_adopt` for a found copy.
-- A machine has one pulled copy of each `model:tag` and one found copy of each key.
--
-- Privileges: unchanged (migration 1042). No row holds a secret or a path.

alter table compute.copies alter column variant_id drop not null;
alter table compute.copies add column origin text not null default 'pulled';
alter table compute.copies add column found text null;
alter table compute.copies add constraint copies_origin_check
    check (origin in ('pulled', 'found'));
alter table compute.copies add constraint copies_found_check
    check (case origin
        when 'found' then found ~ '^[0-9a-f]{64}$' and variant_id is null
        else found is null and variant_id is not null
    end);
alter table compute.copies drop constraint copies_device_model_tag_key;
create unique index copies_device_model_tag_key on compute.copies (device_id, model, tag)
    where origin = 'pulled';
create unique index copies_device_found_key on compute.copies (device_id, found)
    where origin = 'found';
