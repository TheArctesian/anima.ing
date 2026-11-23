alter table notifications.channels drop constraint channels_kind_check;
alter table notifications.channels add constraint channels_kind_check
    check (kind in ('webhook', 'ntfy', 'email', 'apprise', 'mobile'));
