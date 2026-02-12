use futures::FutureExt;
use futures::future::BoxFuture;
use log::{Level, log};
use regex::Regex;
use testcontainers::core::logs::LogFrame;

#[derive(Debug)]
pub struct LogConsumer {
    target: String,
    re: Regex,
}

impl LogConsumer {
    /// Creates a new instance of the logging consumer.
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            re: Regex::new(r"(ERROR|WARNING|WARN|INFO)\:? *\-* *").unwrap(),
        }
    }
}

impl testcontainers::core::logs::consumer::LogConsumer for LogConsumer {
    fn accept<'a>(&'a self, record: &'a LogFrame) -> BoxFuture<'a, ()> {
        async move {
            let msg = String::from_utf8_lossy(record.bytes());
            let level = if msg.contains("ERROR") {
                Level::Error
            } else if msg.contains("WARN") {
                Level::Warn
            } else if msg.contains("INFO") {
                Level::Info
            } else {
                Level::Debug
            };
            if log::log_enabled!(target: &self.target, level) {
                let msg = self.re.replacen(&msg, 1, "");
                let msg = msg.trim_end_matches(['\n', '\r']);
                log!(target: &self.target, level, "{msg}");
            }
        }
        .boxed()
    }
}
