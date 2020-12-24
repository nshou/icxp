pub mod commons;
mod core;

use crate::core::listener::unix_socket_listener::UnixSocketListener;
use std::fs;

fn main() {
    println!("Hello, ICXPd!");

    let c = commons::Commons::init().unwrap();

    fs::create_dir_all(c.get_work_dir().unwrap()).unwrap();

    let l = UnixSocketListener::new(&c).unwrap();
    l.listen().unwrap();
}
