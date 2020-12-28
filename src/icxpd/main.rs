pub mod commons;
mod core;

use crate::commons::Commons;
use crate::core::listener::unix_socket_listener::UnixSocketListener;
use std::fs;

#[tokio::main]
async fn main() {
    println!("Hello, ICXPd!");

    let c = Commons::init().unwrap();

    fs::create_dir_all(c.get_work_dir().unwrap()).unwrap();

    let l = UnixSocketListener::new(&c).unwrap();
    let listener = l.listen();

    //TODO: error handling
    let _rets = tokio::join!(listener);
}
