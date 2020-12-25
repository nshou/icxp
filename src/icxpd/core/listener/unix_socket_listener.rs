use crate::commons::Commons;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

const SOCK_NAME: &str = "icxpd.sock";

pub struct UnixSocketListener {
    sock_path: PathBuf,
    command_sender: Sender<String>,
    running: bool,
}

impl UnixSocketListener {
    pub fn new(c: &Commons) -> Result<UnixSocketListener, String> {
        let work_dir = c
            .get_work_dir()
            .ok_or(String::from("Invalid home directory path"))?;
        let mut sock_path = PathBuf::from(work_dir);
        sock_path.push(SOCK_NAME);
        let command_sender = c.get_command_sender();
        Ok(UnixSocketListener {
            sock_path,
            command_sender,
            running: false,
        })
    }

    pub fn listen(&self) -> std::io::Result<()> {
        let listener = UnixListener::bind(&self.sock_path.as_path())?;
        for stream in listener.incoming() {
            let stream = BufReader::new(stream?);
            for line in stream.lines() {
                //TODO: define own error type and unify the return type here
                self.command_sender.send(line?).unwrap();
            }
        }
        Ok(())
    }
}
