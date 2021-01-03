use crate::commons::Commons;
use std::path::PathBuf;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::Sender;

const SOCK_NAME: &str = "icxpd.sock";

#[derive(Debug)]
pub struct UnixSocketListener<'a> {
    sock_path: PathBuf,
    command_sender: &'a Sender<String>,
}

#[derive(Debug)]
pub enum UnixSocketListenerError {
    Io(io::Error),
    Generic(String),
}

impl UnixSocketListener<'_> {
    pub fn new(c: &Commons) -> Result<UnixSocketListener, UnixSocketListenerError> {
        let work_dir = c
            .get_work_dir()
            .ok_or(UnixSocketListenerError::Generic(String::from(
                "Invalid home directory path",
            )))?;
        let mut sock_path = PathBuf::from(work_dir);
        sock_path.push(SOCK_NAME);
        let command_sender = c.get_command_sender();
        Ok(UnixSocketListener {
            sock_path,
            command_sender,
        })
    }

    pub async fn listen(&self) -> Result<(), UnixSocketListenerError> {
        let listener = UnixListener::bind(&self.sock_path.as_path())?;
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    tokio::spawn(UnixSocketListener::wt_handle(
                        stream,
                        self.command_sender.clone(),
                    ));
                }
                // accepting a connection can lead to various errors
                // and not all of them are necessarily fatal
                //TODO: use logger
                Err(e) => println!("error while accepting: {:?}", e),
            }
        }
        //TODO: gentle shutdown
        // Ok(())
    }

    async fn wt_handle(stream: UnixStream, command_sender: Sender<String>) {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        // multiple input lines work as a batch
        while let Some(line) = lines.next_line().await.unwrap() {
            command_sender.send(line).await.unwrap();
        }
    }
}

impl From<io::Error> for UnixSocketListenerError {
    fn from(err: io::Error) -> UnixSocketListenerError {
        UnixSocketListenerError::Io(err)
    }
}
