ALTER TABLE devices.ssh_settings ADD COLUMN configured boolean NOT NULL DEFAULT false;
ALTER TABLE devices.rdp_settings ADD COLUMN configured boolean NOT NULL DEFAULT false;

UPDATE devices.ssh_settings SET configured = true
WHERE enabled OR username <> '' OR port <> 22;
UPDATE devices.rdp_settings SET configured = true
WHERE enabled OR username <> '' OR port <> 3389 OR domain IS NOT NULL
   OR security <> 'nla' OR ignore_certificate OR resize_method <> 'display_update'
   OR NOT audio_out OR microphone OR shared_drive;

CREATE FUNCTION devices.keep_access_choice() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD IS DISTINCT FROM NEW THEN
        NEW.configured := true;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER ssh_access_choice BEFORE UPDATE ON devices.ssh_settings
FOR EACH ROW EXECUTE FUNCTION devices.keep_access_choice();
CREATE TRIGGER rdp_access_choice BEFORE UPDATE ON devices.rdp_settings
FOR EACH ROW EXECUTE FUNCTION devices.keep_access_choice();
