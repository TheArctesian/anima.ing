-- The audit log records the token kind `command_line` (R-383; K-E5 Section 3b, SGN-NEW-audit):
-- a request of the `anima` program with the token that the desktop app gave it. 0505 named the
-- check inline, so Postgres named it `audit_log_token_kind_check`. No stored row changes
-- (LOG-006), and the runtime role still only reads and appends (0500, R-016).

alter table events.audit_log
    drop constraint audit_log_token_kind_check;

alter table events.audit_log
    add constraint audit_log_token_kind_check
        check (token_kind in ('login', 'browser', 'api_key', 'command_line'));
