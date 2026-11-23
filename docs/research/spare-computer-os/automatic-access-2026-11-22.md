# Automatic private access (OS3-R11)

Anima OS packages a root-owned service declaration. Its daemon checks loopback service availability
and includes available SSH/RDP logins in authenticated inventory announcements. The hub initializes
untouched settings only for that token’s own private-link device. Existing configuration and subsequent
owner changes are retained. The hub records an event when it initializes access. Authentication,
credentials, SSH host-key checks and RDP certificate checks remain unchanged.

Other devices have a username/optional-password setup action, followed by a connection test. Advanced
SSH settings remain available. CLI `anima device update <device> --ssh-username <user>` enables a previously
unconfigured SSH connection with password authentication; explicit flags and configured-device choices win.
No new GraphQL field or MCP secret interface is needed. Automatic installation of a missing host SSH
server is the separate continuation OS3-R12, not a capability of this form.

Deployment requires the updated hub and daemon/OS image; the published 20261123013635 ISO does not
include this change. Deploy the hub before distributing the new OS image. Older hubs ignore the
optional announcement. Hardware and real-account acceptance is HT-NEW-automatic-private-access.
