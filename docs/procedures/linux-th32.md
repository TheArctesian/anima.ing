# Protected personal daemon on Linux

TH32 uses an administrator-installed system unit for remote Code on a personal Linux device.
The supervisor starts the daemon and holder as the selected login user and restarts the daemon
without stopping the holder. It permanently drops each worker's privileges before user state,
login scripts or providers run. The unit stops the whole service on administrative shutdown.
The provider has the person's files and login environment. It cannot gain privileges through a
setuid program or file capabilities.

## Install and enroll

1. Verify the daemon's signed release and install the regular executable as root at
   `/usr/local/libexec/anima-daemon` with mode `0755`. Use a canonical path with root-owned
   directories that other accounts cannot write. This service needs Linux with procfs,
   `fs.suid_dumpable=0`, and permission to permanently drop process credentials and capabilities.
2. Create the daemon's state directory as the person with permissions `0700`, for example
   `/home/alex/.config/anima-daemon`. Keep that directory at the same location across renewals.
   Stop any ordinary user daemon using that directory.
3. Install `crates/daemon/units/anima-th32.system.unit` as `anima-th32.service`. Write `/etc/anima/th32.json`
   with mode `0600`, owned by root. Replace the sample user, home, shell, and numeric IDs
   with the values for the real login account. Supply that account's supplementary group IDs.
   The `authority` path is a private root-owned directory. Its parent must already exist.
4. Generate a fresh K8 code for the device at its HTTPS hub. Run the following in the administrator's
   shell, with the code supplied through a private standard input pipe.

   ```sh
   /usr/local/libexec/anima-daemon linux-supervise /etc/anima/th32.json enrol https://hub.example
   ```

5. Reload the systemd units and start `anima-th32.service`. Use the existing local settings to
   allow remote starts and to select the projects and provider programs. Remote starts default off.

```json
{
  "uid": 1000,
  "gid": 1000,
  "supplementary": [1000],
  "name": "alex",
  "home": "/home/alex",
  "state": "/home/alex/.config/anima-daemon",
  "shell": "/bin/bash",
  "authority": "/var/lib/anima-th32"
}
```

The protected enrollment uses HTTPS with normal certificate and hostname verification. For a private
certificate authority, the administrator can add `hub_trust` with the exact HTTPS `origin` and a
PEM `certificate` to the root-owned configuration.

## Renew and remove

Ordinary renewal runs inside the supervised daemon and retains the enrollment identity.
Revoke the previous credential at the hub when migrating an existing user service. The protected
service creates its own enrollment from a new code. The protected path currently supports HTTPS.

For hardware migration, re-enrollment, or removal, stop `anima-th32.service` first. Revoke the old
device credential at the hub. To remove the locally protected enrollment, run the supervisor
command with `forget` in place of `enrol` and its origin. Re-enroll with a fresh code before starting
the unit again.
