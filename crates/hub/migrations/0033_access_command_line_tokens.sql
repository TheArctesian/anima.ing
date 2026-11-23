-- One sign-in (R-383; K-E5 Section 3b, SGN-NEW-linked): the token of the kind `command_line`
-- that the desktop app asks for, for the `anima` program on its computer. It is linked to the
-- login token of the app: it ends with that token (a `logout`, its expiry, its deletion), and a
-- new sign-in of the same device of a hosted hub moves it to the new token of the app.
-- Numbered after 0032, the last migration of `access`; 0030 named the kind check.

alter table access.api_tokens
    drop constraint api_tokens_kind_check;

alter table access.api_tokens
    add constraint api_tokens_kind_check
        check (kind in ('login', 'browser', 'api_key', 'builder', 'command_line')),
    -- The login token of the app that a command-line token is linked to. Deleting it deletes
    -- the command-line token too, also where the hub deletes tokens with no audit entry (an
    -- expiry prune, `operator set`).
    add column parent_id uuid null references access.api_tokens (id) on delete cascade,
    -- The name of the computer, as the app gave it; for the list of sign-ins and the audit.
    add column computer text null,
    -- The name on the link of the device of a hosted hub that signed in with the account, so
    -- that its next sign-in finds the token that it replaces (HMD-037).
    add column link_device text null,
    add constraint api_tokens_command_line_parent
        check ((kind = 'command_line') = (parent_id is not null)),
    add constraint api_tokens_command_line_computer check (
        case
            when kind = 'command_line' then computer is not null
                and char_length(computer) between 1 and 64
            else computer is null
        end
    ),
    add constraint api_tokens_link_device_login check (
        link_device is null or (kind = 'login' and char_length(link_device) between 1 and 64)
    );

-- One command-line token for each token of the app: a new one replaces the one before.
create unique index api_tokens_one_command_line on access.api_tokens (parent_id)
    where kind = 'command_line';
create index api_tokens_link_device on access.api_tokens (operator_id, link_device)
    where link_device is not null;
