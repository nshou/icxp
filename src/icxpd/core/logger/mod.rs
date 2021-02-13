pub mod null_writer;

use async_trait::async_trait;
use chrono::Local;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};

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
    async fn begin_subscribe(self, receiver: Receiver<LoggerMessage>) -> i32;
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

#[derive(Clone)]
pub enum LoggerMessage {
    Log(LoggerPayload),
    Ctl(LoggerCtl),
}

#[derive(Clone)]
pub enum LoggerCtl {
    Close,
}

#[derive(Clone)]
pub struct LoggerPayload {
    level: Level,
    target: String,
    args: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
}

pub struct Logger {
    publisher: Sender<LoggerMessage>,
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

    pub async fn close(mut self) {
        self.publisher
            .send(LoggerMessage::Ctl(LoggerCtl::Close))
            .ok();
        for writer in self.writers.iter_mut() {
            let mut errmsg = None;
            match time::timeout(Duration::from_millis(writer.1), &mut writer.2).await {
                Ok(joined) => match joined {
                    Ok(retcode) => {
                        if retcode != 0 {
                            errmsg =
                                Some(format!("{} ({})", "returned non-zero when closed", retcode));
                        }
                    }
                    Err(_) => {
                        // tokio::task::JoinError
                        errmsg = Some(String::from(
                            "has already been aborted while trying to close",
                        ));
                    }
                },
                Err(_) => {
                    // tokio::time::error::Elapsed
                    // `Close` could be dropped due to lagging
                    errmsg = Some(String::from(
                        "was forced to shut down due to timeout for closing",
                    ));
                }
            }
            if let Some(msg) = errmsg {
                eprintln!(
                    "{} - [{}] `{}` {}",
                    Local::now().format("%F_%T%.6f"),
                    "WARN",
                    writer.0,
                    msg
                );
            }
        }
    }
}

struct LogDistributor {
    sender: Sender<LoggerMessage>,
}

impl LogDistributor {
    fn new(sender: Sender<LoggerMessage>) -> LogDistributor {
        LogDistributor { sender }
    }
}

impl log::Log for LogDistributor {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn flush(&self) {}

    fn log(&self, record: &Record) {
        let msg = LoggerMessage::Log(LoggerPayload {
            level: record.level(),
            target: String::from(record.target()),
            args: record.args().to_string(),
            module_path: record.module_path().map(|m| String::from(m)),
            file: record.file().map(|f| String::from(f)),
            line: record.line(),
        });
        if self.sender.send(msg).is_err() {
            eprintln!(
                "{} - [{}] {}",
                Local::now().format("%F_%T%.6f"),
                "WARN",
                "No log writers available"
            );
        }
    }
}
