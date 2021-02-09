mod commons;
mod core;

use crate::commons::Commons;
use crate::core::listener::unix_socket_listener::UnixSocketListener;
use crate::core::logger::null_writer::NullWriter;
use crate::core::logger::Logger;
use std::fs;

#[tokio::main]
async fn main() {
    let c = Commons::init(None).unwrap();

    fs::create_dir_all(c.get_work_dir().unwrap()).unwrap();

    let _listener = UnixSocketListener::listen(&c).unwrap();

    let logger = Logger::open().unwrap();
    logger.set_log_writer(NullWriter);
    log::error!("error");

    //TODO: heartbeat loop on current thread
    // immediately exits for now

    //TODO: try_join? select?
    //    let (ul_r,) = tokio::join!(ul);
    //    if let Err(e) = ul_r {
    //        println!("error occurred while joining {:?}: {:?}", l, e);
    //    }
}
