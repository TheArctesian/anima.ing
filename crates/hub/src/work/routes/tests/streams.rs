//! Streams and redirects in the table of routes against PostgreSQL (HOST-050, task H40): one
//! port of the hub for each transport, UDP never through the link, the routes of one
//! application on one device, a redirect with no target, and the streams that serve.

use super::support::{DOCKER_HOST, application, now, settings, shaped, stream};
use crate::{
    ids::DeviceId,
    work::routes::{
        self, Error, NewRoute, Waits,
        kinds::{Access, Kind, KindRefusal, Shape},
    },
};

pub(super) async fn add(pool: &sqlx::PgPool, new: &NewRoute) -> Result<routes::Added, Error> {
    let mut conn = pool.acquire().await.unwrap();
    routes::add((pool, &settings()), &mut conn, new, now()).await
}

pub(super) fn mail(listen_port: i64) -> NewRoute {
    shaped(
        "mail.example.com",
        stream(Kind::TcpStream, Access::Private, listen_port, 1143),
    )
}

/// What the proof, the consent and the record of the application will set.
pub(super) async fn serve_all(pool: &sqlx::PgPool) {
    sqlx::query(
        "UPDATE routes.route SET proof = 'proven', machine_consent = true, \
                                 application_recorded = true",
    )
    .execute(pool)
    .await
    .unwrap();
}

db_test! {
    async fn a_stream_is_stored_with_its_port_of_the_hub_and_needs_no_certificate(pool: PgPool) {
        let route = add(&pool, &mail(20_143)).await.unwrap().route;
        assert_eq!((route.kind, route.access), (Kind::TcpStream, Access::Private));
        assert_eq!(route.listen_port, Some(20_143));
        assert_eq!(route.waits(now()), Some(Waits::Proof), "it waits as each route");
        assert_eq!(routes::get(&pool, route.id).await.unwrap(), route);
        serve_all(&pool).await;
        let stored = routes::get(&pool, route.id).await.unwrap();
        assert_eq!(stored.waits(now()), None, "a stream has no TLS of the hub");
        let streams = routes::served_streams(&pool, now()).await.unwrap();
        assert_eq!(streams.len(), 1);
    }
}

db_test! {
    async fn one_port_of_the_hub_carries_one_stream_of_each_transport(pool: PgPool) {
        add(&pool, &mail(20_143)).await.unwrap();
        let again = shaped("imap.example.com", stream(Kind::TcpStream, Access::Private, 20_143, 1144));
        assert!(matches!(add(&pool, &again).await, Err(Error::ListenPortTaken)));
        let udp = shaped("vpn.example.com", stream(Kind::UdpStream, Access::Private, 20_143, 1194));
        assert!(add(&pool, &udp).await.is_ok(), "UDP on the same port is a different stream");
    }
}

db_test! {
    async fn the_routes_of_an_application_go_to_one_device_on_several_ports(pool: PgPool) {
        add(&pool, &shaped("photos.example.com", super::support::web(3000))).await.unwrap();
        assert!(add(&pool, &mail(20_143)).await.is_ok(), "a second port of the same device");
        let other: DeviceId = "0190a000-0000-7000-8002-000000000003".parse().unwrap();
        let elsewhere = NewRoute { device: other, ..shaped("x.example.com", super::support::web(3001)) };
        assert!(matches!(add(&pool, &elsewhere).await, Err(Error::OtherTarget)));
    }
}

db_test! {
    async fn udp_never_goes_through_the_link(pool: PgPool) {
        sqlx::query("UPDATE devices.devices SET host = 'fd41:4e49:4d41::7' WHERE id = $1::uuid")
            .bind(DOCKER_HOST)
            .execute(&pool)
            .await
            .unwrap();
        let udp = shaped("vpn.example.com", stream(Kind::UdpStream, Access::Private, 20_194, 1194));
        let refused = add(&pool, &udp).await;
        assert!(
            matches!(refused, Err(Error::Kind(KindRefusal::UdpThroughLink))),
            "{refused:?}"
        );
        assert!(add(&pool, &mail(20_143)).await.is_ok(), "TCP goes through the link");
    }
}

db_test! {
    async fn a_redirect_has_no_target_and_is_never_dialled(pool: PgPool) {
        let shape = Shape {
            kind: Kind::Redirect,
            access: Access::Public,
            port: None,
            listen_port: None,
            protocol: None,
            redirect_to: Some("https://photos.example.org/".to_owned()),
        };
        let route = add(&pool, &shaped("old.example.com", shape)).await.unwrap().route;
        assert_eq!((route.port, route.address), (None, None));
        serve_all(&pool).await;
        let found = routes::target_of(&pool, application(), "old.example.com", now()).await;
        assert!(matches!(found, Err(Error::NotFound)));
    }
}

db_test! {
    async fn a_public_stream_waits_for_the_mark_stream_public_of_the_record(pool: PgPool) {
        let imaps = stream(Kind::TcpStream, Access::Public, 20_993, 993);
        let shape = Shape { protocol: Some("imaps".to_owned()), ..imaps };
        let public = add(&pool, &shaped("imap.example.com", shape)).await.unwrap().route;
        add(&pool, &mail(20_143)).await.unwrap();
        serve_all(&pool).await;
        let stored = routes::get(&pool, public.id).await.unwrap();
        assert_eq!(stored.waits(now()), Some(Waits::ApplicationRecord), "the protocol is a label");
        let streams = routes::served_streams(&pool, now()).await.unwrap();
        assert_eq!(streams.len(), 1, "only the private stream serves");
        assert_eq!(streams[0].listen_port, Some(20_143));
        sqlx::query("UPDATE routes.route SET stream_public = true WHERE id = $1")
            .bind(public.id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(routes::get(&pool, public.id).await.unwrap().waits(now()), None);
    }
}

db_test! {
    async fn r_390_a_private_stream_to_a_database_serves_with_no_consent(pool: PgPool) {
        let database = shaped(
            "db.example.com",
            stream(Kind::TcpStream, Access::Private, 20_432, 5432),
        );
        let route = add(&pool, &database).await.unwrap().route;
        sqlx::query("UPDATE routes.route SET proof = 'proven', application_recorded = true")
            .execute(&pool)
            .await
            .unwrap();
        let streams = routes::served_streams(&pool, now()).await.unwrap();
        assert_eq!(streams.len(), 1, "no consent at the machine for a private stream");
        assert_eq!((streams[0].id, streams[0].access), (route.id, Access::Private));
    }
}
