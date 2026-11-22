-- The email address of each account of an AI agent program (K2 1020-1039; THR-149, AIS-022;
-- K8 Section 4 `agentUsage.accounts[].address`).
--
-- The daemon sends the address that the program is signed in with, as T3 Code gives it to each
-- client of the person (THR-149). The address is personal data: it is in no log line and no
-- diagnostics, and deleting a device deletes it with the row of its account (DEV-015). A new
-- report of the device and source replaces it; a report with no address clears it; a report
-- that no longer names the source deletes the row.
--
-- The hub refuses an address of another form before it writes (K8 Section 4), so the check here
-- is of the length only: a check of the alphabet could differ from the hub's with the locale of
-- the database, and a refused row would stop every push of the device and write the address in
-- the log of the database server.
--
-- The column is nullable, so the migration changes no row and is cheap on a large database
-- (DB-010). Privileges are those of migration 1021's table.

alter table agent_usage.accounts
    add column address text,
    add constraint accounts_address_check check (octet_length(address) between 3 and 254);
