pub mod commons;

mod core;
use crate::core::listener::unix_socket_listener::UnixSocketListener;

fn main() {
    println!("Hello, ICXPd!");

    let c = commons::Commons::init().unwrap();

    let l = UnixSocketListener::new(&c).unwrap();
    l.listen().unwrap();
}
