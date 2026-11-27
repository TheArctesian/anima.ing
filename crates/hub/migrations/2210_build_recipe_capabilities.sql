alter table jobs.machines add column build_recipes jsonb not null default 'null';
alter table jobs.decisions drop constraint decisions_verdict_check;
alter table jobs.decisions add constraint decisions_verdict_check check (verdict in (
    'fits', 'type', 'recipe', 'kind', 'os', 'architecture', 'memory', 'disk',
    'refused_before', 'names_other', 'excluded', 'waits_for_better'
));
