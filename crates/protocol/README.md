# goxlr-hub-protocol

The command language of the full-size GoXLR: how each command is laid out in
bytes, and how each answer is read back. This crate never touches hardware,
so every command is covered by a test that checks the exact bytes.

## Credits

The GoXLR protocol is not documented by its manufacturer. This implementation
was written from scratch for GoXLR Hub, using the knowledge gathered by the
[GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility) project
(MIT licence), whose authors reverse-engineered the device. Thank you.
