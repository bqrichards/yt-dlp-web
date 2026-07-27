use tracing::instrument;

#[instrument]
pub async fn healthcheck_endpoint() -> &'static str {
    "OK"
}
