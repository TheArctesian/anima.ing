-- Receipts survive group/project deletion: a lost reply must not undo a later user choice.
create table projects.group_imports (
    source uuid primary key,
    group_id uuid
);
create table projects.group_import_members (
    source uuid not null references projects.group_imports(source),
    project_id uuid not null,
    primary key (source, project_id)
);
