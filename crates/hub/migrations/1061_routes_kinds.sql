-- The kinds and the access of a route (range 1060-1079; HOST-050, task H40; Section 16 of
-- docs/proposals/hosted-applications-stage-2.md).
--
-- `kind`: `web` (HTTPS by name, Section 12), `redirect` (a proven name that answers with a fixed
-- `https://` address, `redirect_to`; it has no port), `tcp_stream` and `udp_stream` (a port of
-- the hub, `listen_port`, to the port of the application; never below 1024). `access`: `public`
-- (the internet) or `private` (only the nodes of the network of Anima, on the address of the hub
-- on that network). The routes of task H21 were public web routes, so the defaults keep them as
-- they were; the hub writes both columns for each new route. `protocol` is the protocol that the
-- person picked for a public stream, from the fixed list of the hub (a protocol with its own
-- sign-in and TLS, SF-248). One port of the hub carries one stream of each transport.
-- `stream_public` says that the hub's record of the application marks this port `stream-public`
-- (SF-NEW-revroutepublic): a public stream waits for it as for the record. Nothing sets it before
-- task H6.
alter table routes.route
    add column kind text not null default 'web'
        check (kind in ('web', 'redirect', 'tcp_stream', 'udp_stream')),
    add column access text not null default 'public'
        check (access in ('public', 'private')),
    add column listen_port integer check (listen_port between 1024 and 65535),
    add column protocol text check (protocol ~ '^[a-z0-9]{1,16}$'),
    add column redirect_to text
        check (char_length(redirect_to) <= 2048 and redirect_to like 'https://%'),
    add column stream_public boolean not null default false,
    alter column port drop not null;

alter table routes.route add constraint route_kind_shape check (
    (kind in ('tcp_stream', 'udp_stream')) = (listen_port is not null)
    and (kind = 'redirect') = (redirect_to is not null)
    and (kind = 'redirect') = (port is null)
);

create unique index route_listen_port
    on routes.route ((kind = 'udp_stream'), listen_port)
    where listen_port is not null;
