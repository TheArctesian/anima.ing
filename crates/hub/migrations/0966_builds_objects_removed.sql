-- BLD-086. A job whose files were removed still counts as published (D-27).
alter table builds.jobs add column objects_removed_at timestamptz null;
