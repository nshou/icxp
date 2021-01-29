use crate::commons::Commons;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc as stdmpsc;
use stdmpsc::TryRecvError;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{self, Duration};

const SOCK_NAME: &str = "icxpd.sock";
const POLL_INTVL_MILLIS: u64 = 10;
const POLL_TRY_COUNT: i32 = 10;

pub struct UnixSocketListener {
    sock_path: PathBuf,
    ctl: stdmpsc::Sender<UnixSocketListenerCtl>,
}

enum UnixSocketListenerCtl {
    Close,
    Nop,
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
                "Failed to look up the path to place Unix socket",
            ))
        })?;
        let mut sock_path = PathBuf::from(work_dir);
        sock_path.push(SOCK_NAME);

        if sock_path.exists() {
            //TODO: use logger
            println!("Socket file already exists. Another icxpd process might be running or crashed last time");
            fs::remove_file(sock_path.as_path()).ok();
        }

        let listener = UnixListener::bind(sock_path.as_path())?;
        let command_sender = c.get_command_sender();
        // Use mpsc of std, not of tokio, because we want try_recv()
        let (ctl_sender, ctl_receiver) = stdmpsc::channel();

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

                match ctl_receiver.try_recv() {
                    Err(ctlerr) => {
                        match ctlerr {
                            TryRecvError::Empty => {}
                            TryRecvError::Disconnected => {
                                // Master thread is dead. Shutdown this thread too
                                break;
                            }
                        }
                    }
                    Ok(ctlcmd) => match ctlcmd {
                        UnixSocketListenerCtl::Close => {
                            break;
                        }
                        UnixSocketListenerCtl::Nop => {}
                    },
                }
            }
        });

        Ok(UnixSocketListener {
            sock_path,
            ctl: ctl_sender,
        })
    }

    pub async fn shutdown(self) -> Result<(), UnixSocketListenerError> {
        let mut stream: Result<UnixStream, io::Error> =
            Err(io::Error::new(io::ErrorKind::Other, "na"));
        // Socket file that listen() creates might not be ready
        for _i in 0..POLL_TRY_COUNT {
            stream = UnixStream::connect(self.sock_path.as_path()).await;
            if let Ok(_) = stream {
                break;
            }
            time::sleep(Duration::from_millis(POLL_INTVL_MILLIS)).await;
        }
        let mut stream = stream?;
        stream.writable().await?;

        if self.ctl.send(UnixSocketListenerCtl::Close).is_err() {
            return Err(UnixSocketListenerError::Generic(String::from(
                "Failed to send a command through control channel",
            )));
        }

        // Ignore errors here. Other normal traffic could cut in first
        stream.try_write("{\"NOP\"}".as_bytes()).ok();
        stream.shutdown().await.ok();

        let mut i = 0;
        while i < POLL_TRY_COUNT && self.ctl.send(UnixSocketListenerCtl::Nop).is_ok() {
            time::sleep(Duration::from_millis(POLL_INTVL_MILLIS)).await;
            i += 1;
        }
        if i == POLL_TRY_COUNT {
            //TODO: use logger
            println!("Gave up waiting for the listener thread to be closed");
        }

        //TODO:
        //When the Receiver is dropped, it is possible for unprocessed messages to remain in the channel. Instead, it is usually desirable to perform a "clean" shutdown. To do this, the receiver first calls close, which will prevent any further messages to be sent into the channel. Then, the receiver consumes the channel to completion, at which point the receiver can be dropped.

        Ok(())
    }
}

impl Drop for UnixSocketListener {
    fn drop(&mut self) {
        fs::remove_file(self.sock_path.as_path()).ok();
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

        let l = UnixSocketListener::listen(&c).unwrap();
        let mut stream: Result<UnixStream, io::Error> =
            Err(io::Error::new(io::ErrorKind::Other, "na"));
        // Socket file that listen() creates might not be ready
        for _i in 0..POLL_TRY_COUNT {
            stream = UnixStream::connect(l.sock_path.as_path()).await;
            if let Ok(_) = stream {
                break;
            }
            time::sleep(Duration::from_millis(POLL_INTVL_MILLIS)).await;
        }
        let mut stream = stream.unwrap();

        stream.writable().await.unwrap();
        assert_eq!(msg.len(), stream.try_write(msg.as_bytes()).unwrap());
        stream.shutdown().await.unwrap();

        assert_eq!(
            Some(String::from(msg)),
            c.get_command_receiver().recv().await
        );

        l.shutdown().await.unwrap();
        fs::remove_dir_all(c.get_work_dir().unwrap()).unwrap();
    }

    //TODO: what if either sender/receiver is closed/shutdown/dropped?
    // cases: sender dropped / receiver dropped / receiver closed
    // e.g. if sender is dropped:
    //   assert_eq!(None, c.get_command_receiver().recv().await);
    // tests should be added after gentle shutdown implemented

    //TODO: panic when thread panics
    //        let (ul_r,) = tokio::join!(ul);
    //        if let Err(e) = ul_r {
    //            println!("error occurred while joining {:?}: {:?}", l, e);
    //        }
}
