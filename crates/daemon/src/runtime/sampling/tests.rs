use super::super::CHANNEL_ROOM;
use super::*;

mod remote_access;

#[tokio::test]
async fn k8_4_sampling_starts_by_announcing_the_gpus() {
    let (sender, mut events) = mpsc::channel(CHANNEL_ROOM);
    let sampling = tokio::spawn(sample(Source::Machine(Platform::Linux), sender));
    let first = tokio::time::timeout(Duration::from_secs(20), events.recv()).await;
    sampling.abort();
    assert!(matches!(first, Ok(Some(Event::Gpus(_)))));
}

#[test]
fn met_006_a_stable_discovery_is_not_repeated_on_the_next_tick() {
    let now = Instant::now();
    let mut watch = Watch::start(Discovery::new(Vec::new()), now);
    assert!(!watch.due(now + SAMPLE_EVERY), "no GPU: soon, not at once");
    watch.replace(Discovery::new(Vec::new()), now);
    assert!(!watch.due(now + SAMPLE_EVERY));
    assert!(watch.due(now + Duration::from_hours(25)));
}
