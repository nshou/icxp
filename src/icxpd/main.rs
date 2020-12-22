pub mod commons;

mod core;
use crate::core::listener::unix_socket_listener::UnixSocketListener;

fn main() {
    println!("Hello, ICXPd!");

    let l = UnixSocketListener::new().unwrap();
    l.listen().unwrap();
}
