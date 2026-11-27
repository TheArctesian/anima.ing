alter table agent_threads.workspace_nodes
    add column private_checkout boolean not null default false;

alter table agent_threads.workspace_requests
    add column checkout_binding jsonb check (octet_length(checkout_binding::text) <= 1024),
    add column checkout_offer_until timestamptz,
    add check (checkout_offer_until is null or (
        checkout_binding is not null and payload->'action'->>'kind' = 'start'
        and checkout_offer_until <= expires_at
    ));

create table agent_threads.workspace_checkout_releases (
    request uuid primary key,
    device uuid not null,
    reserved_at timestamptz not null,
    sealed bytea,
    minted_until timestamptz,
    decision_until timestamptz,
    release_until timestamptz,
    retry_at timestamptz,
    check (sealed is null or minted_until is not null),
    check (decision_until is null or (minted_until is not null and decision_until <= minted_until)),
    check (release_until is null or (minted_until is not null and release_until <= minted_until))
);

create index workspace_checkout_cleanup
    on agent_threads.workspace_checkout_releases(minted_until, request) where sealed is not null;
