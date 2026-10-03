# usb-probe (throwaway spike)

Answers two questions from stage 1: can our own program talk to the GoXLR, and
can it animate a fader display.

The USB protocol comes from [GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility)
(MIT license), used here as a direct dependency.

Before running: quit GoXLR Utility (icon next to the clock, Quit).

    cargo run --manifest-path spikes/usb-probe/Cargo.toml -- info
    cargo run --manifest-path spikes/usb-probe/Cargo.toml -- watch
    cargo run --manifest-path spikes/usb-probe/Cargo.toml -- anim 30

Nothing in this folder is reused as is by the application.
