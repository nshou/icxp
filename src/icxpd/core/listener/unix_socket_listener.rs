use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

const WORK_DIR: &str = ".icxp";
const SOCK_NAME: &str = "icxpd.sock";

pub struct UnixSocketListener {
    sock_path: PathBuf,
    running: bool,
}

impl UnixSocketListener {
    pub fn new() -> Option<UnixSocketListener> {
        let mut path = dirs::home_dir()?;
        path.push(WORK_DIR);
        path.push(SOCK_NAME);
        Some(UnixSocketListener {
            sock_path: path,
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
