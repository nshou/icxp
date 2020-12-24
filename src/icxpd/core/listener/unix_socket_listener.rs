use crate::commons::Commons;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

const SOCK_NAME: &str = "icxpd.sock";

pub struct UnixSocketListener {
    sock_path: PathBuf,
    running: bool,
}

impl UnixSocketListener {
    pub fn new(c: &Commons) -> Result<UnixSocketListener, String> {
        let work_dir = c
            .get_work_dir()
            .ok_or(String::from("Unnable to find home directory"))?;
        let mut sock_path = PathBuf::from(work_dir);
        sock_path.push(SOCK_NAME);
        Ok(UnixSocketListener {
            sock_path,
            running: false,
        })
    }

    pub fn listen(&self) -> std::io::Result<()> {
        let listener = UnixListener::bind(&self.sock_path.as_path())?;
        for stream in listener.incoming() {
            let stream = BufReader::new(stream?);
            for line in stream.lines() {
                println!("{}", line?)
            }
        }
        Ok(())
    }
}
