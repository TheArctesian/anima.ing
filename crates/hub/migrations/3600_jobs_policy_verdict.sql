-- The device policy of the machine stops the model item now (CAP-010 to CAP-012; contract K11,
-- accepted by R-344; task L1-5 of the compute plan). A decision of the placement takes the
-- verdict `policy`.
--
-- This belongs to the range 0980-0999 of `jobs`, but 2210 (the build recipes) replaces the same
-- check after it, so this migration must sort after 2210. The number 3600 is in no range of K2;
-- the operator decides its range (OD-C30 of the compute plan).

alter table jobs.decisions drop constraint decisions_verdict_check;
alter table jobs.decisions add constraint decisions_verdict_check check (verdict in (
    'fits', 'type', 'recipe', 'kind', 'os', 'architecture', 'memory', 'disk',
    'refused_before', 'names_other', 'excluded', 'waits_for_better', 'policy'
));
