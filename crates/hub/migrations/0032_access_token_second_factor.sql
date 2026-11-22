-- The second factor of a login, asked again before a reveal of a sealed record (K-E5 Section 3,
-- VLT-005): when the token last passed a TOTP code, and the wrong codes that it gave since.
alter table access.api_tokens
    add column totp_at timestamptz,
    add column totp_failures smallint not null default 0 check (totp_failures >= 0);

comment on column access.api_tokens.totp_at is
    'When this token last passed a TOTP code: at its sign-in or at a reveal (VLT-005).';
comment on column access.api_tokens.totp_failures is
    'Wrong TOTP codes that this token gave for a reveal since it last passed one; at 5 the token is deleted.';
