use std::path::PathBuf;
use tokio::sync::mpsc::{self, Receiver, Sender};

const WORK_DIR_NAME: &str = ".icxp";
const COMMAND_BRIDGE_QLEN: usize = 128;

pub struct Commons {
    work_dir: PathBuf,
    command_bridge: (Sender<String>, Receiver<String>),
}

impl Commons {
    //TODO: unify error struct
    pub fn init() -> Result<Commons, String> {
        let mut work_dir =
            dirs::home_dir().ok_or(String::from("Unnable to find home directory"))?;
        work_dir.push(WORK_DIR_NAME);
        let command_bridge = mpsc::channel(COMMAND_BRIDGE_QLEN);
        Ok(Commons {
            work_dir,
            command_bridge,
        })
    }

    pub fn get_work_dir(&self) -> Option<&str> {
        self.work_dir.to_str()
    }

    pub fn get_command_sender(&self) -> &Sender<String> {
        &self.command_bridge.0
    }
}
