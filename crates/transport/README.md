# goxlr-hub-transport

Carries the bytes of a command to a GoXLR and brings the answer back. It
knows nothing about what the commands mean: that is the `protocol` crate.

- **Windows**: through the library of the official TC-Helicon driver, loaded
  at run time. The driver must be installed; nothing else is.
- **Linux**: through libusb. Built and tested by CI, not tried on hardware
  yet.

It also tells which model is plugged in, and whether another program that
drives the GoXLR (GoXLR Utility, the official app) is running: the Windows
driver lets several programs open the device at once, and they would then
steal each other's answers.

## Credits

How to reach the device, on both systems, comes from the work of the
[GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility) project
(MIT licence). This implementation was written for GoXLR Hub. Thank you.
