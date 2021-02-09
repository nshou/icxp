use crate::core::logger::LogWriter;
use async_trait::async_trait;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;

pub struct NullWriter;

#[async_trait]
impl LogWriter for NullWriter {
    fn get_writer_name(&self) -> String {
        String::from("Null Writer")
    }

    async fn begin_subscribe(self, mut receiver: Receiver<String>) -> i32 {
        loop {
            match receiver.recv().await {
                Err(RecvError::Closed) => break,
                _ => {}
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::logger::Logger;

    #[tokio::test(flavor = "multi_thread")]
    async fn log_five_levels() {
        let mut logger = Logger::open().unwrap();
        logger.set_log_writer(NullWriter);
        log::error!("error");
        log::warn!("warn");
        log::info!("info");
        log::debug!("debug");
        log::trace!("trace");

        //TODO: close and teardown
    }
}
