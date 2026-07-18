use std::{
    path::PathBuf,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use tracing::{debug, warn};

pub struct DeleteOnDropStream<S> {
    inner: S,
    path: Option<PathBuf>,
}

impl<S> DeleteOnDropStream<S> {
    pub fn new(inner: S, path: PathBuf) -> Self {
        Self {
            inner,
            path: Some(path),
        }
    }

    fn cleanup(&mut self) {
        if let Some(path) = self.path.take() {
            tokio::spawn(async move {
                match tokio::fs::remove_file(&path).await {
                    Ok(()) => {
                        debug!("Deleted {:?}", path);
                    }
                    Err(e) => {
                        warn!("Failed to delete {:?}: {}", path, e);
                    }
                }
            });
        }
    }
}

impl<S, T> Stream for DeleteOnDropStream<S>
where
    S: Stream<Item = T> + Unpin,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(None) => {
                self.cleanup();
                Poll::Ready(None)
            }
            other => other,
        }
    }
}

impl<S> Drop for DeleteOnDropStream<S> {
    fn drop(&mut self) {
        self.cleanup();
    }
}
