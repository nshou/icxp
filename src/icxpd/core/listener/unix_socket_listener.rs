use crate::commons::Commons;
use std::path::{Path, PathBuf};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::Sender;
use tokio::time::{self, Duration};

const SOCK_NAME: &str = "icxpd.sock";

#[derive(Debug)]
pub struct UnixSocketListener {
    sock_path: PathBuf,
    command_sender: Sender<String>,
    state: UnixSocketListenerState,
}

#[derive(Debug)]
enum UnixSocketListenerState {
    Initialized,
    Running,
    Closing,
    Closed,
}

#[derive(Debug)]
pub enum UnixSocketListenerError {
    Io(io::Error),
    Generic(String),
}

impl UnixSocketListener {
    pub fn new(c: &Commons) -> Result<UnixSocketListener, UnixSocketListenerError> {
        let work_dir = c.get_work_dir().ok_or_else(|| {
            UnixSocketListenerError::Generic(String::from("Invalid home directory path"))
        })?;
        let mut sock_path = PathBuf::from(work_dir);
        sock_path.push(SOCK_NAME);
        let command_sender = c.get_command_sender();
        Ok(UnixSocketListener {
            sock_path,
            command_sender,
            state: UnixSocketListenerState::Initialized,
        })
    }

    pub fn get_sock_path(&self) -> Option<&str> {
        self.sock_path.to_str()
    }

    pub async fn listen(&mut self) -> Result<(), UnixSocketListenerError> {
        let listener = UnixListener::bind(&self.sock_path.as_path())?;
        self.state = UnixSocketListenerState::Running;
        while let UnixSocketListenerState::Running = self.state {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    tokio::spawn(UnixSocketListener::wt_handle(
                        stream,
                        self.command_sender.clone(),
                    ));
                }
                // Accepting a connection can lead to various errors
                // and not all of them are necessarily fatal
                //TODO: use logger
                Err(e) => println!("error while accepting: {:?}", e),
            }
        }
        self.state = UnixSocketListenerState::Closed;
        Ok(())
    }

    async fn wt_handle(stream: UnixStream, command_sender: Sender<String>) {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        // Multiple input lines work as a batch
        while let Some(line) = lines.next_line().await.unwrap() {
            command_sender.send(line).await.unwrap();
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), UnixSocketListenerError> {
        //TODO: early return based on self.state

        let sock_path = String::from(self.get_sock_path().ok_or_else(|| {
            UnixSocketListenerError::Generic(String::from("Invalid socket file path"))
        })?);
        let mut stream: Result<UnixStream, io::Error> =
            Err(io::Error::new(io::ErrorKind::Other, "na"));
        // Socket file that l.listen() creates might not be ready
        for _i in 0_i32..10 {
            time::sleep(Duration::from_millis(10)).await;
            stream = UnixStream::connect(Path::new(&sock_path)).await;
            if let Ok(_) = stream {
                break;
            }
        }
        let mut stream = stream?;

        stream.writable().await?;
        self.state = UnixSocketListenerState::Closing;
        // Ignore errors here. Other normal traffic could come in first
        stream.try_write("{\"NOP\"}".as_bytes()).ok();
        stream.shutdown().await.ok();

        Ok(())
    }
}

impl From<io::Error> for UnixSocketListenerError {
    fn from(err: io::Error) -> UnixSocketListenerError {
        UnixSocketListenerError::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn reserve_test_dir_name() -> String {
        format!(".icxp_test_{}", Uuid::new_v4().to_hyphenated().to_string())
    }

    #[tokio::test]
    async fn one_message_delivery() {
        let msg = "one_message_delivery";
        let test_dir_name = reserve_test_dir_name();
        let mut c = Commons::init(Some(&test_dir_name)).unwrap();
        fs::create_dir_all(c.get_work_dir().unwrap()).unwrap();

        let mut l = UnixSocketListener::new(&c).unwrap();
        let sock_path = String::from(l.get_sock_path().unwrap());
        let listener = tokio::spawn(async move {
            l.listen().await.unwrap();
        });

        let mut stream: Result<UnixStream, io::Error> =
            Err(io::Error::new(io::ErrorKind::Other, "na"));
        // Socket file that l.listen() creates might not be ready
        for _i in 0_i32..10 {
            time::sleep(Duration::from_millis(10)).await;
            stream = UnixStream::connect(Path::new(&sock_path)).await;
            if let Ok(_) = stream {
                break;
            }
        }
        // Let it panic if failed for 10 times
        let mut stream = stream.unwrap();

        stream.writable().await.unwrap();
        assert_eq!(msg.len(), stream.try_write(msg.as_bytes()).unwrap());
        stream.shutdown().await.unwrap();

        assert_eq!(
            Some(String::from(msg)),
            c.get_command_receiver().recv().await
        );

        //TODO: what if either sender/receiver is closed?
        // cases: sender dropped / receiver dropped / receiver closed
        // e.g. if sender is dropped:
        //   assert_eq!(None, c.get_command_receiver().recv().await);
        // tests should be added after gentle shutdown implemented

        //TODO: panic when thread panics
        //        let (ul_r,) = tokio::join!(ul);
        //        if let Err(e) = ul_r {
        //            println!("error occurred while joining {:?}: {:?}", l, e);
        //        }

        //TODO: teardown
        //close listener and delete test dir
    }
}
