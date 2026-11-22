-- What the daemon of a device waits for from a person at the device (K2 0710-0729; R-364).
--
-- `daemons.need` is set by the desktop app of that computer, which reads the mark file of its
-- daemon (R-322), and cleared by the app, by the next accepted batch and by a new token. Null:
-- nothing known. `keychain`: macOS waits for the person to allow the daemon in the Keychain.
--
-- One nullable column with no default: no table rewrite, cheap on a large database (DB-010).

alter table metrics.daemons add column need text null check (need in ('keychain'));
