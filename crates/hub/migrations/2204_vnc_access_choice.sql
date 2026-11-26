-- Old disabled rows have no provenance: an upgrade cannot infer permission.
ALTER TABLE devices.vnc_settings ADD COLUMN configured boolean NOT NULL DEFAULT false;
UPDATE devices.vnc_settings SET configured = true;

CREATE TRIGGER vnc_access_choice BEFORE UPDATE ON devices.vnc_settings
FOR EACH ROW EXECUTE FUNCTION devices.keep_access_choice();
