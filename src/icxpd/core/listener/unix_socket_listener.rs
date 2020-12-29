use crate::commons::Commons;
use crate::core::command::Command;
use std::path::PathBuf;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::Sender;

const SOCK_NAME: &str = "icxpd.sock";

#[derive(Debug)]
pub struct UnixSocketListener<'a> {
    sock_path: PathBuf,
    command_sender: &'a Sender<Command>,
}

impl UnixSocketListener<'_> {
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
        })
    }

    pub async fn listen(&self) -> io::Result<()> {
        let listener = UnixListener::bind(&self.sock_path.as_path())?;
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    tokio::spawn(UnixSocketListener::handle(
                        stream,
                        self.command_sender.clone(),
                    ));
                }
                //TODO: better error handling
                Err(e) => println!("error: {:?}", e),
            }
        }
    }

    //TODO: consider making funcs below module level impl

    async fn handle(stream: UnixStream, command_sender: Sender<Command>) {
        let reader = BufReader::new(stream);
        let mut lines = reader.lines();
        // multiple input lines work as a batch
        //TODO: define own error type and unify the return type here
        //TODO: remove unnecessary async mode
        while let Some(line) = lines.next_line().await.unwrap() {
            command_sender
                .send(UnixSocketListener::parse(&line))
                .await
                .unwrap();
        }
    }

    fn parse(args: &String) -> Command {
        //TODO:
        Command::NOP(args.clone())
    }
}
