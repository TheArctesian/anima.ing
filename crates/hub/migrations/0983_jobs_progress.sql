-- The progress of a running `model_pull` (K2 range 0980-0999; contract K11 Section 3, PULL-007;
-- task L1-7 of the compute plan): the last `progress` member of a report of the holder, as
-- `{ "bytes", "totalBytes", "bytesPerSecond" }`. Null for each other job, and before the first
-- report with progress. It holds numbers only.

alter table jobs.jobs
    add column progress jsonb null
        constraint jobs_progress_check check (jsonb_typeof(progress) = 'object');
