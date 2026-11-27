use std::time::Duration;

use anima_daemon::threads::wire::{Action, Event};

#[tokio::test]
async fn th32_same_user_client_cannot_open_the_remote_holder_exchange() {
    let holder = super::super::Holder::start(0);
    let mut talk = holder.link().await;
    talk.link
        .send(Action::RemoteCode {
            frame: r#"{"op":"open"}"#.to_owned(),
        })
        .await
        .unwrap();
    let watch = async {
        while let Some(event) = talk.link.events.recv().await {
            assert!(
                !matches!(event, Event::RemoteCode { .. }),
                "same-user test process received a remote grant challenge"
            );
        }
    };
    assert!(
        tokio::time::timeout(Duration::from_millis(350), watch)
            .await
            .is_err()
    );
}
