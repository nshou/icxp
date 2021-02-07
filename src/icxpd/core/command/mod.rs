//TODO: Should these really be published as mod.rs?
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub enum Command {
    NOP(Nop),
}

#[derive(Deserialize, Debug)]
pub struct Nop {
    extras: Option<String>,
}

impl Command {
    pub fn from_json(j: &String) -> Result<Command, String> {
        //TODO: unify errors and propagate serde_json::Result
        let cmd: Result<Command, _> = serde_json::from_str(&j);
        cmd.or(Err(String::from("json parser error")))
    }
}
