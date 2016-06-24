'use strict';

const net = require('net');
const assert = require('assert');

const counter = 10000;
let tmpCounter = 1;
let time = 0;

function callback() {
    if (tmpCounter++ === counter) {
        console.log('Needed time:', Date.now() - time, 'ms');
    }
}

time = Date.now();

for (let i = 0; i < counter; i++) {
    const text = 'I am the ' + i + ' one';
    const client = net.connect({port: 8888}, () => {
        client.write(text);
    });
    
    client.on('data', (buffer) => {
        assert.equal(buffer.toString(), text);
        client.end();
    });
    
    client.on('end', callback);
}