'use strict';

const net = require('net');

const connections = [];

function removeSocket(socket) {
    const index = connections.indexOf(socket);

    if (index > -1) {
        connections.splice(index, 1);
        console.log('Got ' + connections.length + 'connections left');
    }
}

const server = net.createServer({ allowHalfOpen: false }, (socket) => {
    connections.push(socket);

    socket.on('error', () => removeSocket(socket));
    socket.on('close', () => removeSocket(socket));

    socket.on('data', (buffer) => {
        socket.write(buffer);
    });
});

server.listen(8888);