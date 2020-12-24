use std::path::PathBuf;

const WORK_DIR_NAME: &str = ".icxp";

pub struct Commons {
    work_dir: PathBuf,
}

impl Commons {
    pub fn init() -> Result<Commons, String> {
        let mut work_dir =
            dirs::home_dir().ok_or(String::from("Unnable to find home directory"))?;
        work_dir.push(WORK_DIR_NAME);

        Ok(Commons { work_dir })
    }

    pub fn get_work_dir(&self) -> Option<&str> {
        self.work_dir.to_str()
    }
}
