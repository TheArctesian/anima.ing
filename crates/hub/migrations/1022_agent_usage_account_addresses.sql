-- The email address of each account of an AI agent program (K2 1020-1039; THR-149, AIS-022;
-- K8 Section 4 `agentUsage.accounts[].address`).
--
-- The daemon sends the address that the program is signed in with, as T3 Code gives it to each
-- client of the person (THR-149). The address is personal data: it is in no log line and no
-- diagnostics, and deleting a device deletes it with the row of its account (DEV-015). A new
-- report of the device and source replaces it; a report with no address clears it.
--
-- The column is nullable, so the migration changes no row and is cheap on a large database
-- (DB-010). Privileges are those of migration 1021's table.

alter table agent_usage.accounts
    add column address text,
    add constraint accounts_address_check check (
        octet_length(address) between 3 and 254
        and address ~ '^[^@[:space:][:cntrl:]<>]+@[^@[:space:][:cntrl:]<>]+$'
    );
