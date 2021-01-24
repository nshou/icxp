use crate::commons::Commons;
use std::path::{Path, PathBuf};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::{self, Duration};

const SOCK_NAME: &str = "icxpd.sock";
const CTL_BRIDGE_QLEN: usize = 8;

pub struct UnixSocketListener {
    sock_path: PathBuf,
    state: UnixSocketListenerState,
    ctl: Sender<UnixSocketListenerCtl>,
}

enum UnixSocketListenerState {
    Running,
    Closing,
    Closed,
}

enum UnixSocketListenerCtl {
    Close,
}

#[derive(Debug)]
pub enum UnixSocketListenerError {
    Io(io::Error),
    Generic(String),
}

impl UnixSocketListener {
    pub fn listen(c: &Commons) -> Result<UnixSocketListener, UnixSocketListenerError> {
        let work_dir = c.get_work_dir().ok_or_else(|| {
            UnixSocketListenerError::Generic(String::from(
                "Failed to look up the path to place Unix socket file",
            ))
        })?;
        let mut sock_path = PathBuf::from(work_dir);
        sock_path.push(SOCK_NAME);
        let listener = UnixListener::bind(sock_path.as_path())?;
        let command_sender = c.get_command_sender();
        let (ctl_sender, ctl_receiver) = mpsc::channel(CTL_BRIDGE_QLEN);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let command_sender_clone = command_sender.clone();
                        tokio::spawn(async move {
                            let reader = BufReader::new(stream);
                            let mut lines = reader.lines();
                            // Multiple input lines work as a batch
                            //TODO: use logger instead of unwrap
                            while let Some(line) = lines.next_line().await.unwrap() {
                                //TODO: use logger instead of unwrap
                                command_sender_clone.send(line).await.unwrap();
                            }
                        });
                    }
                    // Accepting a connection can lead to various errors
                    // and not all of them are necessarily fatal
                    Err(e) => {
                        //TODO: use logger
                        println!("error while accepting: {:?}", e);
                    }
                }

                //TODO: ctl match
            }
        });

        Ok(UnixSocketListener {
            sock_path,
            state: UnixSocketListenerState::Running,
            ctl: ctl_sender,
        })
    }
}

impl From<io::Error> for UnixSocketListenerError {
    fn from(err: io::Error) -> UnixSocketListenerError {
        UnixSocketListenerError::Io(err)
    }
}
