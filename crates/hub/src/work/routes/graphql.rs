//! The routes GraphQL part: root `Query` and `Mutation` (K1; HOST-046, task H21).
//!
//! A read key may read the routes: no field holds a certificate or a key. Each change needs a
//! login or a browser token: the three mutations are in `API_KEY_REFUSED_ROOTS`, and each
//! resolver checks the kind of the token again, so no API key and no builder token changes a
//! route. One rule for each caller (operator ruling of 2026-11-22 on adding a route): the person, the app, `anima` and an agent
//! add and remove a route with no approval; a public route still serves only after the consent
//! at the machine (OD-HOST-43). Adding and checking a route also need a second factor on the
//! account (SF-232, HM-D14). Removing one needs only the signed-in token: a removal only takes a
//! name and its reach away, and it cannot change, point again or make a route. On a hub that is
//! not on Unix the routes cannot serve, so adding and checking a route is refused: "Routing
//! needs a hub on Linux." (R-366). Each change writes one audit entry and one event in its own
//! transaction.

mod trail;
mod types;

use std::{str::FromStr, sync::Arc};

use async_graphql::{Context, ID, Object, Result};

use self::types::{AddedView, RouteFields, RouteView, shape};
use super::{Error, NewRoute, Settings, SystemDns, kinds::KindRefusal, proof::Resolver};
use crate::{
    api_errors::{
        ErrorCode, FieldError, begin, coded, commit, internal, invalid, pool, schema_dependency,
    },
    clock::Clock,
    config::Config,
    fleet::{
        access::{self, Authenticated, operator_only},
        devices,
        events::Trail,
        targets::AddressPolicy,
    },
};

/// The DNS of the checks: the context's, when a test gives one; the system's otherwise.
#[derive(Clone)]
pub struct Dns(pub Arc<dyn Resolver>);

fn settings(ctx: &Context<'_>) -> Result<Settings> {
    let config = schema_dependency::<Arc<Config>>(ctx)?;
    let policy = schema_dependency::<AddressPolicy>(ctx)?;
    Ok(Settings::new(config, policy.clone()))
}

fn now(ctx: &Context<'_>) -> Result<chrono::DateTime<chrono::Utc>> {
    Ok(schema_dependency::<Arc<dyn Clock>>(ctx)?.now())
}

fn parse<T: FromStr>(id: &ID, field: &'static str) -> Result<T> {
    id.parse()
        .map_err(|_| invalid(&[FieldError::new(field, "Not a valid ID.")]))
}

/// The K7 form of a route error. No message holds a secret.
fn to_graphql(error: Error) -> async_graphql::Error {
    match error {
        Error::NotFound => coded("No such route.", ErrorCode::NotFound),
        Error::NoDevice => coded("No such device.", ErrorCode::NotFound),
        Error::Name(refusal) => invalid(&[FieldError::new("name", refusal.to_string())]),
        Error::Target(refusal) => invalid(&[FieldError::new("port", refusal.to_string())]),
        Error::Kind(refusal) => invalid(&[FieldError::new(
            refusal_field(&refusal),
            refusal.to_string(),
        )]),
        Error::ListenPortTaken => invalid(&[FieldError::new("listenPort", error.to_string())]),
        Error::Unresolved => invalid(&[FieldError::new("deviceId", error.to_string())]),
        Error::NameTaken | Error::OtherTarget => coded(error.to_string(), ErrorCode::Conflict),
        Error::NeedsLinuxHub => coded(error.to_string(), ErrorCode::Unavailable),
        other => internal(other),
    }
}

/// The input field that a refusal of the kind names.
const fn refusal_field(refusal: &KindRefusal) -> &'static str {
    match refusal {
        KindRefusal::NoListenPort
        | KindRefusal::ListenPortNotForKind
        | KindRefusal::NoStreamPorts
        | KindRefusal::ListenPortBelow1024
        | KindRefusal::ListenPortOutsideRange
        | KindRefusal::ListenPortReserved => "listenPort",
        KindRefusal::StreamOnHostedHub | KindRefusal::RedirectShape => "kind",
        KindRefusal::UdpThroughLink | KindRefusal::NoPort => "port",
        KindRefusal::PublicStreamProtocol(_) | KindRefusal::ProtocolNotForKind => "protocol",
        KindRefusal::RedirectToNotForKind | KindRefusal::RedirectTo => "redirectTo",
    }
}

/// A person with a login or browser token, whose account has a second factor (HM-D14).
async fn person_with_second_factor(ctx: &Context<'_>, root: &str) -> Result<()> {
    operator_only(ctx, root)?;
    let status = access::operator_status(pool(ctx)?)
        .await
        .map_err(internal)?;
    if status.is_some_and(|status| status.totp) {
        Ok(())
    } else {
        Err(coded(
            "Turn on the second factor of the account before a route is added or checked.",
            ErrorCode::Forbidden,
        ))
    }
}

#[derive(Default)]
pub struct Query;

#[Object(name = "RouteQuery")]
impl Query {
    #[graphql(guard = "Authenticated")]
    async fn application_routes(
        &self,
        ctx: &Context<'_>,
        application_id: Option<ID>,
    ) -> Result<Vec<RouteView>> {
        let application = application_id
            .map(|id| parse(&id, "applicationId"))
            .transpose()?;
        let rule = settings(ctx)?.proof;
        let now = now(ctx)?;
        let routes = super::list(pool(ctx)?, application)
            .await
            .map_err(to_graphql)?;
        Ok(routes
            .into_iter()
            .map(|route| RouteView {
                route,
                rule: rule.clone(),
                now,
            })
            .collect())
    }

    #[graphql(guard = "Authenticated")]
    async fn application_route(&self, ctx: &Context<'_>, id: ID) -> Result<Option<RouteView>> {
        let id = parse(&id, "id")?;
        match super::get(pool(ctx)?, id).await {
            Ok(route) => Ok(Some(RouteView {
                route,
                rule: settings(ctx)?.proof,
                now: now(ctx)?,
            })),
            Err(Error::NotFound) => Ok(None),
            Err(error) => Err(to_graphql(error)),
        }
    }
}

#[derive(Default)]
pub struct Mutation;

#[Object(name = "RouteMutation")]
impl Mutation {
    #[graphql(guard = "Authenticated")]
    async fn add_application_route(
        &self,
        ctx: &Context<'_>,
        input: RouteFields,
    ) -> Result<AddedView> {
        super::routing_here(cfg!(unix)).map_err(to_graphql)?;
        let shape = shape(&input);
        person_with_second_factor(ctx, "addApplicationRoute").await?;
        let new = NewRoute {
            application: parse(&input.application_id, "applicationId")?,
            device: parse(&input.device_id, "deviceId")?,
            shape,
            name: input.name,
        };
        let settings = settings(ctx)?;
        let trail = Trail::from_context(ctx)?;
        let db = pool(ctx)?;
        let mut tx = begin(ctx).await?;
        let added = super::add((db, &settings), &mut tx, &new, trail.now)
            .await
            .map_err(to_graphql)?;
        let device = devices::get(db, added.route.device)
            .await
            .map_err(internal)?;
        let entry = trail::added(&added.route, &device);
        let now = trail.now;
        trail.write(&mut tx, entry).await.map_err(internal)?;
        commit(tx).await?;
        Ok(AddedView {
            added,
            rule: settings.proof,
            now,
        })
    }

    #[graphql(guard = "Authenticated")]
    async fn check_application_route(&self, ctx: &Context<'_>, id: ID) -> Result<RouteView> {
        super::routing_here(cfg!(unix)).map_err(to_graphql)?;
        person_with_second_factor(ctx, "checkApplicationRoute").await?;
        let id = parse(&id, "id")?;
        let settings = settings(ctx)?;
        let resolver: Arc<dyn Resolver> = match ctx.data_opt::<Dns>() {
            Some(Dns(resolver)) => Arc::clone(resolver),
            None => Arc::new(SystemDns::new().map_err(internal)?),
        };
        let trail = Trail::from_context(ctx)?;
        let mut tx = begin(ctx).await?;
        let route = super::check(&mut tx, (resolver.as_ref(), &settings), id, trail.now)
            .await
            .map_err(to_graphql)?;
        let device = devices::get(pool(ctx)?, route.device)
            .await
            .map_err(internal)?;
        let now = trail.now;
        trail
            .write(&mut tx, trail::checked(&route, &device))
            .await
            .map_err(internal)?;
        commit(tx).await?;
        Ok(RouteView {
            route,
            rule: settings.proof,
            now,
        })
    }

    #[graphql(guard = "Authenticated")]
    async fn remove_application_route(&self, ctx: &Context<'_>, id: ID) -> Result<ID> {
        operator_only(ctx, "removeApplicationRoute")?;
        let route = parse(&id, "id")?;
        let trail = Trail::from_context(ctx)?;
        // The target of a route never changes after it is added (SF-233), so the route read
        // before the delete is the route that the delete takes away.
        let removed = super::get(pool(ctx)?, route).await.map_err(to_graphql)?;
        let mut tx = begin(ctx).await?;
        let (device, _) = super::remove(&mut tx, route).await.map_err(to_graphql)?;
        let device = devices::get(pool(ctx)?, device).await.map_err(internal)?;
        trail
            .write(&mut tx, trail::removed(&removed, &device))
            .await
            .map_err(internal)?;
        commit(tx).await?;
        Ok(id)
    }
}
