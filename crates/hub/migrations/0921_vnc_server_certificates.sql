-- Independent VNC certificate approvals; existing RDP identities remain unchanged.
create table trust.vnc_server_certificates (
    device_id uuid primary key,
    fingerprint_sha256 text not null,
    subject text null,
    not_after timestamptz null,
    first_seen_at timestamptz not null,
    accepted_by uuid not null,
    constraint vnc_server_certificates_device_id_fkey foreign key (device_id)
        references devices.devices (id) on delete cascade,
    constraint vnc_server_certificates_accepted_by_fkey foreign key (accepted_by)
        references access.operators (id),
    constraint vnc_server_certificates_fingerprint_check check (
        fingerprint_sha256 ~ '^([0-9a-f]{2}:){31}[0-9a-f]{2}$'
    ),
    -- The device chooses its subject: the hub keeps at most 1024 characters of it and no
    -- control character, so it cannot break a line wherever it is shown.
    constraint vnc_server_certificates_subject_check check (
        subject is null
        or (char_length(subject) between 1 and 1024 and subject !~ '[[:cntrl:]]')
    ),
    -- Real instants only, as `sessions_times_finite` for session rows.
    constraint vnc_server_certificates_times_finite check (
        isfinite(first_seen_at) and (not_after is null or isfinite(not_after))
    )
);

revoke delete, update on trust.vnc_server_certificates from anima_runtime;
grant update (fingerprint_sha256, subject, not_after, first_seen_at, accepted_by)
    on trust.vnc_server_certificates to anima_runtime;
