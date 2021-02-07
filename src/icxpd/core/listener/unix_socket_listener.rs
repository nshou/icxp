use crate::commons::Commons;
use std::fs;
use std::io::ErrorKind;
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
        //TODO: from trait
        if self.ctl.send(UnixSocketListenerCtl::Close).is_err() {
            return Err(UnixSocketListenerError::Generic(String::from(
                "Failed to send a command through control channel",
            )));
        }

        let mut i = 0;
        while i < POLL_TRY_COUNT {
            let conn = UnixStream::connect(self.sock_path.as_path()).await;
            match conn {
                Ok(mut stream) => {
                    stream.writable().await?;
                    // Ignore errors here. Other normal traffic could cut in first
                    stream.try_write("{\"NOP\"}".as_bytes()).ok();
                    stream.shutdown().await.ok();
                    if self.ctl.send(UnixSocketListenerCtl::Nop).is_err() {
                        // Ctl receiver dropped, meaning the listener successfully ended
                        break;
                    }
                }
                Err(err) => {
                    match err.kind() {
                        ErrorKind::NotFound => { /* Socket not bound yet. Continue */ }
                        ErrorKind::ConnectionRefused => {
                            // Socket closed, meaning the listener successfully ended
                            break;
                        }
                        _ => {
                            //TODO: from trait
                            return Err(UnixSocketListenerError::Io(err));
                        }
                    }
                }
            }
            time::sleep(Duration::from_millis(POLL_INTVL_MILLIS)).await;
            i += 1;
        }

        if i == POLL_TRY_COUNT {
            //TODO: use logger
            println!("Gave up waiting for the listener thread to be closed");
        }

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
    use std::path::Path;
    use uuid::Uuid;

    fn _reserve_test_dir_name() -> String {
        format!(".icxp_test_{}", Uuid::new_v4().to_hyphenated().to_string())
    }

    fn prepare() -> Option<Commons> {
        let test_dir_name = _reserve_test_dir_name();
        let c = Commons::init(Some(&test_dir_name)).ok()?;
        fs::create_dir_all(c.get_work_dir()?).ok()?;
        Some(c)
    }

    fn teardown(c: Commons) -> Option<()> {
        fs::remove_dir_all(&c.get_work_dir()?).ok()?;
        Some(())
    }

    async fn connect(path: &Path) -> Option<UnixStream> {
        for _i in 0..POLL_TRY_COUNT {
            let conn = UnixStream::connect(&path).await;
            if let Ok(stream) = conn {
                return Some(stream);
            }
            time::sleep(Duration::from_millis(POLL_INTVL_MILLIS)).await;
        }
        None
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn one_message_delivery() {
        let msg = "one_message_delivery";
        let mut c = prepare().unwrap();
        let l = UnixSocketListener::listen(&c).unwrap();
        let mut stream = connect(l.sock_path.as_path()).await.unwrap();

        stream.writable().await.unwrap();
        assert_eq!(msg.len(), stream.try_write(msg.as_bytes()).unwrap());
        stream.shutdown().await.unwrap();

        assert_eq!(
            Some(String::from(msg)),
            c.get_command_receiver().recv().await
        );

        l.shutdown().await.unwrap();
        teardown(c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn batch_message_delivery() {
        let mut c = prepare().unwrap();
        let l = UnixSocketListener::listen(&c).unwrap();
        let mut stream = connect(l.sock_path.as_path()).await.unwrap();

        let mut ssum: u32 = 0;
        let mut rsum: u32 = 0;
        let mut _istr = String::new();
        stream.writable().await.unwrap();
        for i in 0..100 {
            _istr = format!("{}\n", i.to_string());
            assert_eq!(_istr.len(), stream.try_write(_istr.as_bytes()).unwrap());
            ssum += i;
        }
        stream.shutdown().await.unwrap();

        for _ in 0..100 {
            _istr = c.get_command_receiver().recv().await.unwrap();
            rsum += _istr.parse::<u32>().unwrap();
        }

        assert_eq!(ssum, rsum);

        l.shutdown().await.unwrap();
        teardown(c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn many_messages_delivery() {
        let mut c = prepare().unwrap();
        let l = UnixSocketListener::listen(&c).unwrap();

        let mut ssum: u32 = 0;
        for i in 0..100 {
            let sock_path_str = String::from(l.sock_path.to_str().unwrap());
            tokio::spawn(async move {
                let istr = i.to_string();
                let mut stream = connect(Path::new(&sock_path_str)).await.unwrap();
                stream.writable().await.unwrap();
                assert_eq!(istr.len(), stream.try_write(istr.as_bytes()).unwrap());
                stream.shutdown().await.unwrap();
            });
            ssum += i;
        }

        let mut rsum: u32 = 0;
        for _ in 0..100 {
            let istr = c.get_command_receiver().recv().await.unwrap();
            rsum += istr.parse::<u32>().unwrap();
        }

        assert_eq!(ssum, rsum);

        l.shutdown().await.unwrap();
        teardown(c);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn allow_dup_listeners() {
        let msg0 = "msg0";
        let msg1 = "msg1";
        let mut c0 = prepare().unwrap();
        let mut c1 = prepare().unwrap();
        let l0 = UnixSocketListener::listen(&c0).unwrap();
        let mut stream0 = connect(l0.sock_path.as_path()).await.unwrap();
        let l1 = UnixSocketListener::listen(&c1).unwrap();
        let mut stream1 = connect(l1.sock_path.as_path()).await.unwrap();

        stream0.writable().await.unwrap();
        assert_eq!(msg0.len(), stream0.try_write(msg0.as_bytes()).unwrap());
        stream1.writable().await.unwrap();
        assert_eq!(msg1.len(), stream1.try_write(msg1.as_bytes()).unwrap());
        stream0.shutdown().await.unwrap();
        stream1.shutdown().await.unwrap();

        assert_eq!(
            Some(String::from(msg0)),
            c0.get_command_receiver().recv().await
        );
        assert_eq!(
            Some(String::from(msg1)),
            c1.get_command_receiver().recv().await
        );

        l0.shutdown().await.unwrap();
        l1.shutdown().await.unwrap();
        teardown(c0);
        teardown(c1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn immediate_shutdown() {
        let c = prepare().unwrap();
        let l = UnixSocketListener::listen(&c).unwrap();
        l.shutdown().await.unwrap();
        teardown(c);
    }

    //TODO: what if either sender/receiver is closed/shutdown/dropped?
    // cases: sender dropped / receiver dropped / receiver closed
    // e.g. if sender is dropped:
    //   assert_eq!(None, c.get_command_receiver().recv().await);
    // tests should be added after gentle shutdown implemented
}
