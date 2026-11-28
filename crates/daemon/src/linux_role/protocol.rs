use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize as _, Zeroizing};

#[derive(Deserialize, Serialize)]
pub(super) enum Request {
    Credential,
    Store(Update),
    Forget,
    Holder,
    HolderIdentity,
    HolderRoot,
    Journal(super::journal::Request),
}

#[derive(Deserialize, Serialize)]
pub(super) struct Update {
    pub(super) record: Vec<u8>,
    pub(super) fresh: Option<Uuid>,
}

impl Drop for Update {
    fn drop(&mut self) {
        self.record.zeroize();
    }
}

#[derive(Deserialize, Serialize)]
pub(super) enum Reply {
    Done,
    Missing,
    Refused,
    Credential(Credential),
    HolderIdentity(Identity),
    HolderRoot(Vec<u8>),
    Journal(super::journal::Reply),
}

#[derive(Deserialize, Serialize)]
pub(super) struct Credential {
    pub(super) record: Vec<u8>,
    pub(super) device: Uuid,
    pub(super) seed: [u8; 32],
}

impl Credential {
    pub(super) fn into_parts(mut self) -> (Zeroizing<Vec<u8>>, Uuid, Zeroizing<[u8; 32]>) {
        (
            Zeroizing::new(std::mem::take(&mut self.record)),
            self.device,
            Zeroizing::new(self.seed),
        )
    }
}

impl Drop for Credential {
    fn drop(&mut self) {
        self.record.zeroize();
        self.seed.zeroize();
    }
}

#[derive(Deserialize, Serialize)]
pub(super) struct Identity {
    pub(super) root: Vec<u8>,
    pub(super) leaf: crate::threads::holder_channel::Leaf,
}
