use super::{Duration, Instant, recovered, sample_is_fresh, some_avg10};

#[test]
fn a_short_quiet_period_does_not_clear_hosting_pressure() {
    let start = Instant::now();
    let mut quiet_since = None;
    for (elapsed, sample, open) in [
        (0, "2.50", false),
        (5, "0.12", false),
        (30, "0.12", false),
        (34, "0.75", false),
        (35, "0.12", false),
        (64, "0.12", false),
        (65, "0.12", true),
    ] {
        let psi = format!(
            "some avg10={sample} avg60=0.20 avg300=0.10 total=1200\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n"
        );
        assert_eq!(
            recovered(
                &mut quiet_since,
                start + Duration::from_secs(elapsed),
                some_avg10(&psi).unwrap()
            ),
            open,
            "sample at {elapsed}s",
        );
    }
}

#[test]
fn a_clear_pressure_observation_expires_and_cannot_come_from_the_future() {
    let sampled_at = 100;
    assert!(sample_is_fresh(Some(sampled_at), 114));
    assert!(!sample_is_fresh(Some(sampled_at), 115));
    assert!(!sample_is_fresh(Some(sampled_at), 99));
}
