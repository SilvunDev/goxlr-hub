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
compressor, the equaliser and the de-esser, next to a live level meter, and
each of them goes back to neutral with one button. Without a GoXLR it shows a
built-in virtual device, in demo mode, where profiles can be prepared.

Settings reach the GoXLR at once and are saved when you ask: a banner says
when something is not saved, and quitting asks first. A profile is made of
pieces saved apart, a mix (faders, volumes, routing) and a microphone, so that
one can be changed without the other; controls and lighting will join them.
Mutes are not part of a profile. Profiles are plain TOML files in the
configuration folder of the app.

Each file says which format it is written in. A file made by a newer version
of the app is neither loaded nor overwritten by an older one. The first time
another version of the app starts, it copies the profiles to the `backups`
folder next to them before touching anything; the ten latest copies are kept.

The app tells when a new version is out and installs nothing by itself.
Settings lists every published version: on Windows one click installs the one
you choose, newer or older, after checking the download against the checksum
GitHub publishes; on Linux it opens the download page. To know about new
versions the app asks GitHub for the list of releases when it starts, then
once a day. Nothing about you or your GoXLR is sent, and Settings turns it
off.

On launch the app brings the GoXLR to the last profile. The first time, it
makes a profile that leaves the volumes as they are, and the microphone type
and gain too until you choose them; every channel starts live, on the default
faders (Mic, Chat, Music, System), with a default routing and a neutral gate,
compressor and equaliser.

The app can start with the computer (Settings). Started that way it stays in
the system tray unless you choose otherwise.

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
- `core`: what the app knows about the device, which device it shows, and
  the profiles.
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
