use std::borrow::Cow;

use futures::FutureExt;
use futures::future::BoxFuture;
use testcontainers::core::logs::LogFrame;
use testcontainers::core::logs::consumer::LogConsumer;

/// A consumer that logs the output of container with the [`log`] crate.
///
/// By default, both standard out and standard error will both be emitted at INFO level.
#[derive(Debug, Default)]
pub struct TracingConsumer {
    prefix: String,
}

impl TracingConsumer {
    /// Creates a new instance of the logging consumer.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    fn format_message<'a>(&self, message: &'a str) -> Cow<'a, str> {
        let message = message.trim_end_matches(['\n', '\r']);
        Cow::Owned(format!("[{}] {}", self.prefix, message))
    }
}

impl LogConsumer for TracingConsumer {
    fn accept<'a>(&'a self, record: &'a LogFrame) -> BoxFuture<'a, ()> {
        async move {
            match record {
                LogFrame::StdOut(bytes) => {
                    tracing::debug!("{}", self.format_message(&String::from_utf8_lossy(bytes)));
                }
                LogFrame::StdErr(bytes) => {
                    tracing::debug!("{}", self.format_message(&String::from_utf8_lossy(bytes)));
                }
            }
        }
        .boxed()
    }
}
