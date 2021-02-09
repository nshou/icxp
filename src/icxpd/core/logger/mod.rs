pub mod null_writer;

use async_trait::async_trait;
use chrono::Local;
use log::{LevelFilter, Metadata, Record, SetLoggerError};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::task::JoinHandle;

const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Error;
const LOG_LEVEL_ENV_KEY: &str = "ICXPD_LOG_LEVEL";
const LOG_LINE_QLEN: usize = 1_000_000;

//TODO: remove async_trait once it's officially supported
#[async_trait]
pub trait LogWriter {
    fn get_writer_name(&self) -> String;
    fn get_join_timeout_millis(&self) -> u64 {
        1000
    }
    async fn begin_subscribe(self, receiver: Receiver<String>) -> i32;
}

#[derive(Debug)]
pub enum LoggerError {
    //TODO: more types with 'from' traits
    Generic(String),
}

impl From<SetLoggerError> for LoggerError {
    fn from(_err: SetLoggerError) -> LoggerError {
        LoggerError::Generic(String::from("A logger has already been set"))
    }
}

pub struct Logger {
    publisher: Sender<String>,
    writers: Vec<(String, u64, JoinHandle<i32>)>,
}

impl Logger {
    pub fn open() -> Result<Logger, LoggerError> {
        let level = match std::env::var(LOG_LEVEL_ENV_KEY) {
            Ok(lv) => match lv.to_lowercase().as_str() {
                "trace" => LevelFilter::Trace,
                "debug" => LevelFilter::Debug,
                "info" => LevelFilter::Info,
                "warn" => LevelFilter::Warn,
                _ => DEFAULT_LOG_LEVEL,
            },
            _ => DEFAULT_LOG_LEVEL,
        };

        let (sender, _) = broadcast::channel(LOG_LINE_QLEN);
        let publisher = sender.clone();
        let ldr = LogDistributor::new(sender);

        log::set_max_level(level);
        log::set_boxed_logger(Box::new(ldr))?;

        Ok(Logger {
            publisher,
            writers: Vec::new(),
        })
    }

    pub fn set_log_writer(&mut self, writer: (impl LogWriter + 'static)) {
        let name = writer.get_writer_name();
        let timeout = writer.get_join_timeout_millis();
        let receiver = self.publisher.subscribe();
        let handle = tokio::spawn(writer.begin_subscribe(receiver));
        self.writers.push((name, timeout, handle));
    }

    //TODO: close()
    // closing: All sender dropped -> next recv() -> RecvError::Closed -> exit inf. loop and end thread
    // should wait for all writers to be closed -> need to keep JoinHandles
}

//TODO: broadcast enum (log, ctl, etc), not String

struct LogDistributor {
    sender: Sender<String>,
}

impl LogDistributor {
    fn new(sender: Sender<String>) -> LogDistributor {
        LogDistributor { sender }
    }
}

impl log::Log for LogDistributor {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn flush(&self) {}

    fn log(&self, record: &Record) {
        let line = format!(
            "{} - [{}] {}:{} {} - {}",
            Local::now().format("%F_%T%.6f"),
            record.level(),
            record.file().unwrap_or("?file?"),
            record.line().unwrap_or(0),
            record.target(),
            record.args()
        );
        if self.sender.send(line).is_err() {
            let eline = format!(
                "{} - [{}] {}",
                Local::now().format("%F_%T%.6f"),
                "WARN",
                "No log writers available"
            );
            eprintln!("{}", eline);
        }
    }
}
