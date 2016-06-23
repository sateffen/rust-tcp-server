extern crate mio;

mod connection;
mod server;

use server::Server;
use std::str::FromStr;
use mio::EventLoop;
use mio::tcp::TcpListener;

fn main() {
    let addr = FromStr::from_str("127.0.0.1:8888").ok().expect("Failed to parse host:port string");
    let listener = TcpListener::bind(&addr).ok().expect("Failed to bind address");
    let mut event_loop = EventLoop::new().ok().expect("Failed to create event loop");

    let mut server = Server::new(listener);
    server.register(&mut event_loop).ok().expect("Failed to register server with event loop");

    event_loop.run(&mut server).ok().expect("Failed to start event loop");
}
