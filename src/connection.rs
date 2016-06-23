use server::Server;
use std::io::{Read, Write, Result};
use std::vec::Vec;
use mio::{EventLoop, Token, EventSet, PollOpt};
use mio::tcp::TcpStream;

// this struct represents a connection from the outsite.
// This is a pretty simple description by holding the socket
// the token, and a list of interests (events we want to receive).
// additionally we hold an queue of things to send, that we can work
// through every time we can write
pub struct Connection {
    socket: TcpStream,
    token: Token,
    interest: EventSet,
    send_queue: Vec<Vec<u8>>,
}

// then we implement the methods for a connection
impl Connection {
    // first we need a constructor, that creates everything we need
    pub fn new(socket: TcpStream, token: Token) -> Connection {
        Connection {
            socket: socket,
            token: token,
            // we need a default eventset, and the simplest is to listen for
            // hung up sockets
            interest: EventSet::hup(),
            send_queue: Vec::new(),
        }
    }

    // this method is called every time the socket is readable
    pub fn readable(&mut self) -> Result<()> {
        // so first we init a buffer list. The name is not that good, but it's the
        // buffer where the complete received data gets written to. So it's a bytearray
        // of ALL received bytes
        let mut buffer_list: Vec<u8> = Vec::new();
        // then we create a temporary buffer, that is 1kb long (1024 bytes), and that gets
        // used to receive the data
        let mut tmp_buffer: [u8; 1024] = [0; 1024];

        // then we loop endlessly through the read, because we need to read ALL data. The
        // eventloop is used in OneShot mode, so it'll notify us only once. If we don't read
        // everything, this will kill the socket
        loop {
            // so basically we have to read the data into the tmp_buffer
            match self.socket.read(&mut tmp_buffer) {
                // and if reading was successful
                Ok(len) => {
                    // we go for every byte and push it to the buffer_list for the final result
                    for i in 0..len {
                        buffer_list.push(tmp_buffer[i]);
                    }

                    // then, if we received less than a complete buffer, we assume there is nothing
                    // left to read.
                    if len < 1024 {
                        break;
                    }
                },
                // else there was an error, so we simply return the error
                Err(e) => {
                    return Err(e);
                }
            }
        }

        // then, just for debugging, we convert the received buffer to an utf8 string
        let buffer_as_string = String::from_utf8(buffer_list.clone()).ok().expect("Received buffer could not be parsed");
        // and print it, so we know something happend
        println!("Received: {}", buffer_as_string);

        // because we echo everything, we simply send the received buffer back (just try it, it's not important currently)
        try!(self.send_message(buffer_list));

        // and if we reach this, everything went smoothly, so tell the outer world: Ok
        Ok(())
    }

    // this method gets called every time the socket is writeable. Every time it's writeable, we look
    // into the send_queue and send the next item to send
    pub fn writable(&mut self) -> Result<()> {
        let mut buffer = match self.send_queue.pop() {
            // if there is buffer to send, simply take it
            Some(buffer) => buffer,
            // else there is nothing to send, so just return ok.
            // actually, we might treat this as error as well, because this can
            // never happen (we have only an interest in writing if send_message
            // was called), but it's nothing super critical. Maybe log it?
            None => {
                return Ok(());
            }
        };

        // then we try to send the buffer
        match self.socket.write_all(&mut buffer) {
            // and if it was ok
            Ok(_) => {},
            Err(e) => {
                // else there was an error with the socket, to tell the outer world
                return Err(e);
            }
        }

        // then, if we don't want to send anything anymore, we simply remove our interest from
        // sending things
        if self.send_queue.is_empty() {
            self.interest.remove(EventSet::writable());
        }

        // and finally simply return an Ok
        Ok(())
    }

    // queues things up for sending, and adds the interest to send
    pub fn send_message(&mut self, message: Vec<u8>) -> Result<()> {
        self.send_queue.push(message);
        self.interest.insert(EventSet::writable());
        Ok(())
    }

    //and register the socket (and add the interest to read, because we want to read
    // every time we register)
    pub fn register(&mut self, event_loop: &mut EventLoop<Server>) -> Result<()> {
        self.interest.insert(EventSet::readable());

        event_loop.register(
            &self.socket,
            self.token,
            self.interest, 
            PollOpt::edge() | PollOpt::oneshot()
        )
    }

    // and add a simple reregister
    pub fn reregister(&mut self, event_loop: &mut EventLoop<Server>) -> Result<()> {
        event_loop.reregister(
            &self.socket,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot()
        )
    }
}