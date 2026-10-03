<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.svg">
    <img src="assets/brand/logo-light.svg" alt="GoXLR Hub" width="334">
  </picture>
</p>

# GoXLR Hub

An open source control app for the TC-Helicon GoXLR, built around plugins.

> **Unofficial project.** GoXLR Hub is not affiliated with, endorsed by, or
> supported by TC-Helicon. "GoXLR" is a trademark of its owner and is used
> here only to describe the hardware this software works with.

https://github.com/user-attachments/assets/a97958d0-c720-4854-b838-77a2eee86ec4

## Status

Early development. The app opens, shows its frame in English or French and
lives in the system tray. It connects to a full-size GoXLR and drives its
mixer in both directions: volumes, mutes and the channel under each fader are
set from the screen, and the screen follows the faders and mute buttons of the
device. The routing grid chooses which source goes to which output. The
microphone section sets the microphone type and gain, the noise gate, the
compressor, the equaliser and the de-esser, next to a live level meter.
Profiles are not there yet. Without a GoXLR it shows a built-in virtual
device, in demo mode.

When it takes the GoXLR over, the app sends the mutes, the fader assignment,
the routing and the microphone processing, which the device cannot tell: every
channel starts live, on the default faders (Mic, Chat, Music, System), with a
default routing and a neutral gate, compressor and equaliser. Volumes are left
as they are, and so are the microphone type and gain until you choose them.

The GoXLR keeps no settings of its own. When it is plugged in again it plays
with everything open until the app takes it back, a fraction of a second
later: a source that was muted or routed away can be heard for that moment.

- **Windows**: the official TC-Helicon driver must be installed.
- **Linux**: built and tested by CI, not tried on a real device yet.
- GoXLR Utility and the official app must be closed: the GoXLR can only be
  driven by one program at a time. GoXLR Hub tells you when one is running.

## Goals

- A modern interface for the full-size GoXLR on Windows and Linux.
- Every button, pad, encoder and fader can be reassigned to any action.
- Fine-grained microphone and channel audio control.
- Lighting and fader display customisation.
- A sandboxed plugin system: sound catalogues, audio effects, integrations.

## Roadmap

Work is tracked with [milestones](../../milestones), one per stage:

0. Repository setup
1. Feasibility spikes
2. Core: device control
3. Controls: gestures, macros, actions
4. Lighting and fader displays
5. Plugin system
6. First-party plugins
7. Public release

## Development

You need [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/) 24,
[pnpm](https://pnpm.io/) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```bash
pnpm install
pnpm tauri dev
```

The Rust code lives in `crates/`, the interface (Svelte) in `ui/`:

- `protocol`: the command language of the device, with no hardware access.
- `transport`: carries commands to the device (official driver on Windows,
  libusb on Linux).
- `device`: one device interface, with the real or the virtual device behind
  it.
- `core`: what the app knows about the device, and which device it shows.
- `app`: the desktop shell.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Brand

Logo, colours and typefaces are described in
[assets/brand](assets/brand/README.md).

## Credits

Knowledge of the device protocol, and of how to reach the device on each
system, comes from
[GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility) (MIT).

## License

GoXLR Hub is licensed under the
[GNU General Public License v3.0 or later](LICENSE).

Plugins that only talk to GoXLR Hub through its public plugin interface are
separate works and may use any license.
