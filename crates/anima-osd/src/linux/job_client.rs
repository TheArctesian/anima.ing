use super::{
    Error, client,
    jobs::schema::{Operation, Outcome, Reply, Request},
};
use std::{path::Path, time::Duration};

pub async fn request(path: &Path, operation: Operation) -> Result<(), Error> {
    let request = Request {
        version: 2,
        operation,
    };
    let bytes = serde_json::to_vec(&request)?;
    let bytes = tokio::time::timeout(Duration::from_secs(4), client::exchange(path, bytes))
        .await
        .map_err(|_| Error::Policy("local job request timed out"))??;
    let reply: Reply = serde_json::from_slice(&bytes)?;
    let matches = match (&request.operation, &reply.outcome) {
        (_, Outcome::Error(_)) | (Operation::List { .. }, Outcome::Operations { .. }) => true,
        (
            Operation::Start { request_id, .. } | Operation::Get { request_id },
            Outcome::Operation(record),
        ) => record.request_id == *request_id,
        (Operation::Cancel { operation_id }, Outcome::Operation(record)) => {
            record.id == *operation_id
        }
        _ => false,
    };
    if reply.version != 2 || !matches {
        return Err(Error::Policy("incompatible local job reply"));
    }
    println!("{}", serde_json::to_string(&reply)?);
    if matches!(reply.outcome, Outcome::Error(_)) {
        return Err(Error::Policy("local job request refused"));
    }
    Ok(())
}
