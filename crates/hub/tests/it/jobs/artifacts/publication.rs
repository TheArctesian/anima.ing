use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anima_hub::{
    clock::Clock,
    work::artifacts::{Delivery, DeliveryError},
};
use chrono::{DateTime, TimeDelta, Utc};

use super::{PVE_01, PgPool, Store, Work, not_retained, offered, ready};

struct ExpireAtPublication {
    active: DateTime<Utc>,
    published: PathBuf,
    expired: AtomicBool,
}

impl Clock for ExpireAtPublication {
    fn now(&self) -> DateTime<Utc> {
        if self.published.exists() {
            self.expired.store(true, Ordering::SeqCst);
        }
        if self.expired.load(Ordering::SeqCst) {
            self.active + TimeDelta::seconds(91)
        } else {
            self.active
        }
    }
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn an_expiry_during_publication_leaves_no_retained_archive(pool: PgPool) {
    let work = Work::new(&pool).await;
    let (_, job) = ready(&work).await;
    let offer = offered(&job);
    let device = PVE_01.parse().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("artifacts");
    let published = root.join(format!("{}-{}", offer.job, offer.attempt));
    let store = Store::open(&root, offer.bytes, 1).unwrap();
    let clock: Arc<dyn Clock> = Arc::new(ExpireAtPublication {
        active: work.now(),
        published: published.clone(),
        expired: AtomicBool::new(false),
    });
    let delivery = Delivery {
        store: &store,
        db: &pool,
        clock,
    };
    let pending = delivery
        .prepare(device, offer.clone(), b"abc".as_slice())
        .await
        .unwrap();

    assert!(matches!(
        delivery.retain(pending).await,
        Err(DeliveryError::NotHolder)
    ));
    assert!(
        !published.exists(),
        "the expired publication still owns a retained archive"
    );
    not_retained(&store, offer).await;
}
