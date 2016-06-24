use connection::Connection;
use std::io;
use mio::{EventLoop, Token, EventSet, PollOpt, Handler};
use mio::tcp::TcpListener;
use mio::util::Slab;

// define a struct, that is our server. It contains of the socket we're listening at
// the server token itself, and a Slab of connections
pub struct Server {
    socket: TcpListener,
    token: Token,
    connections: Slab<Connection>,
}

// then we need to impmlement everything the Handler-trait of mio tells us to implement
impl Handler for Server {
    // first define this two types. I don't know what they are for, so simply make them
    // empty. This is valid, and works
    type Timeout = ();
    type Message = ();

    // then we define the ready-method. This function gets called every time the eventloop
    // has something to do for us. The eventloop gets called with the corresponding token,
    // as well as all events that occured
    fn ready(&mut self, event_loop: &mut EventLoop<Server>, token: Token, events: EventSet) {
        // if there was an error, or the socket hung up
        if events.is_error() || events.is_hup() {
            // close the connection and quit processing the event
            self.close_connection(event_loop, token);
            return;
        }

        // if the event is a writeable event
        if events.is_writable() {
            // it has to be NOT the server socket that is writeable. A server socket (TcpListener)
            // is NEVER writeable! The assert! will panic
            assert!(self.token != token, "Received writable event for Server");

            // else the assertion was right, so the writeable event is for a connection socket.
            // Now we need to find the connection. Actually I would love to cache the connection
            // and reuse it later in the callback, but the borrow checker hits hard when doing that
            self.find_connection_by_token(token)
                // tell it about the writeable event
                .writable()
                // and tell it to reregister
                .and_then(|_| self.find_connection_by_token(token).reregister(event_loop))
                // and if something goes wrong, we close the connection. Maybe handle the error?
                .unwrap_or_else(|_| {
                    self.close_connection(event_loop, token);
                });
        }

        // else if the event is readable
        if events.is_readable() {
            // if the server is readable
            if self.token == token {
                // just accept the socket
                self.accept(event_loop);
            }
            // else we have a readable event from another socket
            else {
                // so find the connection
                self.find_connection_by_token(token)
                    // tell it about the readable thing
                    .readable()
                    // and reregister it back to the eventloop
                    .and_then(|_| self.find_connection_by_token(token).reregister(event_loop))
                    // else, if something went wrong, close the connection. Maybe handle the error?
                    .unwrap_or_else(|_| {
                        self.close_connection(event_loop, token);
                    });
            }
        }
    }
}
// now we implemented the handler trait for the Server struct.
// so now we need to implement the Server struct itself
impl Server {
    // first we create a new method, that only takes the TcpListener to use. Everything
    // else is creatable on the fly
    pub fn new(socket: TcpListener) -> Server {
        Server {
            socket: socket,
            // we start at Token(1) because some people wrote kqueue (an event system
            // under mio) sometimes make stupid things
            token: Token(1),
            // and because we start the Token(1), we have to tell the Slab to start at
            // Token(2) and enable it to go up to 16384 (so 16382 connections)
            connections: Slab::new_starting_at(Token(2), 16384),
        }
    }

    // this function registers this Server to the eventloop
    pub fn register(&mut self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
        match event_loop.register(&self.socket,
                                  self.token,
                                  EventSet::readable(),
                                  PollOpt::edge() | PollOpt::oneshot()) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    // this function reregisters the Server to the eventloop
    pub fn reregister(&mut self, event_loop: &mut EventLoop<Server>) {
        match event_loop.reregister(&self.socket,
                                    self.token,
                                    EventSet::readable(),
                                    PollOpt::edge() | PollOpt::oneshot()) {
            Ok(_) => {}
            // maybe log the error?
            Err(_) => {
                event_loop.shutdown();
            }
        }
    }

    // this function gets called, whenever there is a socket connection to accept
    // this will read the socket and handle it
    fn accept(&mut self, event_loop: &mut EventLoop<Server>) {
        // first extract the socket to receive by accepting the TcpListener
        let socket = match self.socket.accept() {
            // and if the accept was successful
            Ok(s) => {
                // we need to check if the socket still exists (don't ask why, but we
                // have to do it)
                match s {
                    // and if we found something, we return it
                    Some((socket, _)) => socket,
                    // else just reregister to the eventloop and do nothing, because
                    // there is nothing
                    None => {
                        self.reregister(event_loop);
                        return;
                    }
                }
            }
            // else if there was an error, just reregister to the eventloop and
            // do nothing. Maybe log the error?
            Err(_) => {
                self.reregister(event_loop);
                return;
            }
        };
        // now, after this long block, we have an actual socket. Now we can work
        // with the socket

        // so now we say the connections Slab, that we want to insert something to
        // get a place to do so
        match self.connections.insert_with(|token| {
            // and because we have a place for the socket, we create a connection
            // struct and return it
            Connection::new(socket, token)
        }) {
            // if the insert was successful
            Some(token) => {
                // we register the token to the eventloop
                match self.find_connection_by_token(token).register(event_loop) {
                    // and do nothing else
                    Ok(_) => {}
                    // else there was an error. Maybe log the error?
                    Err(_) => {
                        // so we remove the socket again. It was never added to the
                        // eventloop, so no error event will ever occur, so we need to
                        // clean up
                        self.connections.remove(token);
                    }
                }
            }
            // else we can't add something to the Slab anymore, so tellt
            None => {
                println!("Slab is full, can't accept any connections anymore");
            }
        };

        // and finally, we need to reregister to the loop, because we actually handled an event
        // on the server, and we need to reregister the event receiver
        self.reregister(event_loop);
    }

    // and we need a close connection function. This will close given connection with given
    // token (ok, not closing, but remove it from the Slab and so from the eventloop)
    // if the passed token is the server itself, we shutdown the eventloop, because something
    // went wrong
    fn close_connection(&mut self, event_loop: &mut EventLoop<Server>, token: Token) {
        if self.token == token {
            event_loop.shutdown();
        } else {
            match self.find_connection_by_token(token).close(event_loop) {
                Ok(_) => {}
                Err(e) => {
                    println!("Could not deregister token {:?} {:?}", token, e);
                }
            };
            self.connections.remove(token);
            println!("Closed a connection, got {} connections left",
                     self.connections.count());
        }
    }

    // this function returns a lifetime mutable connection. To be honest, I don't fully get it,
    // but this way we can work with the connections without any problems.
    fn find_connection_by_token<'a>(&'a mut self, token: Token) -> &'a mut Connection {
        &mut self.connections[token]
    }
}
