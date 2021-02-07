use crate::core::logger::LogWriter;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

pub struct NullLogger;

impl LogWriter for NullLogger {
    fn subscribe(&self, mut receiver: Receiver<String>) {
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Err(RecvError::Closed) => break,
                    _ => {}
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::logger::Logger;

    #[tokio::test(flavor = "multi_thread")]
    async fn log_five_levels() {
        let logger = Logger::open().unwrap();
        let nulll = NullLogger;
        logger.set_log_writer(&nulll);
        log::error!("error");
        log::warn!("warn");
        log::info!("info");
        log::debug!("debug");
        log::trace!("trace");

        //TODO: close and teardown
    }
}
