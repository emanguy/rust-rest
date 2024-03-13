use axum::body;
use serde::de::DeserializeOwned;

/// Used in tests to both extract the raw bytes from the HTTP response body and then deserialize them into the
/// requested type. Will panic and fail the test if either step fails somehow.
pub async fn deserialize_body<T: DeserializeOwned>(response_body: body::Body) -> T {
    let bytes = body::to_bytes(response_body, usize::MAX)
        .await
        .expect("Could not read data from response body!");

    serde_json::from_slice(&bytes).unwrap_or_else(|err| {
        panic!(
            "Could not parse body content into data structure! Error: {}, Received body: {:?}",
            err, bytes
        )
    })
}
