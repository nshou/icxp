pub mod commons;
mod core;

use crate::commons::Commons;
use crate::core::listener::unix_socket_listener::UnixSocketListener;
use std::fs;

#[tokio::main]
async fn main() {
    let c = Commons::init(None).unwrap();

    fs::create_dir_all(c.get_work_dir().unwrap()).unwrap();

    let l = UnixSocketListener::new(&c).unwrap();
    let ul = l.listen();

    //TODO: try_join? select?
    let (ul_r,) = tokio::join!(ul);
    if let Err(e) = ul_r {
        println!("error occurred while joining {:?}: {:?}", l, e);
    }
}
