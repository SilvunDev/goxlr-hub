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
each of them goes back to neutral with one button. The Controls screen chooses
what each of the 24 buttons does, for four ways of pressing it: short, long,
double, or held. For now the actions mute a track or the microphone, cut a
track toward one output only (a cell of the routing grid), set, raise or lower
a volume, switch the pad bank, and switch profile, or only one piece of a
profile (mute and cut actions can last while the button is held). The four
dials can each be given the volume of a track. Without a GoXLR it shows a
built-in virtual device, in demo mode, where profiles can be prepared and where
the buttons and dials of the Controls screen can be clicked.

Settings reach the GoXLR at once and are saved when you ask: a banner says
when something is not saved, and quitting asks first. A profile is made of
pieces saved apart, a mix (faders, volumes, routing), a microphone and the
controls (what each button does), so that one can be changed without the
others; lighting will join them. Mutes are not part of a profile. Profiles are
plain TOML files in the configuration folder of the app.

A button that has a double press waits a third of a second before its short
press goes, to be sure no second press follows. A button with a hold has no
other gesture. The buttons work as long as the app runs, window closed or not.

A button that switches profile does it at once and does not ask: changes that
are not saved are lost, and the Controls screen says so when the action is
chosen. The place of a dial in its travel is the volume it sets: the bottom is
0%, the top is 100%, the ring of lights of the dial shows it, and the dial
stops by itself at both ends. Each notch moves the volume by 100% divided by
the number of notches of the travel, which the app measures for about a second
when the dial is given a job. When the volume changes by other means (the
screen, a button, a fader, a profile, another dial), the app puts the dial
where the new volume is. A dial that the device does not put where the volume
is is only followed by how far it turns, a few percent a notch, and can then
stop at the end of its travel; the Controls screen says so. A volume the app
does not know yet (a track on no fader that was never set) is left alone by a
button that raises or lowers it, and its dial is not touched: set it once
first.

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

## Install

Download the file for your system from the
[latest release](https://github.com/SilvunDev/goxlr-hub/releases/latest).

| System | File |
|---|---|
| Windows 10 and 11 (64-bit) | `goxlr-hub_<version>_windows_x64-setup.exe` |
| Debian, Ubuntu and relatives | `goxlr-hub_<version>_linux_amd64.deb` |
| Fedora, openSUSE and relatives | `goxlr-hub_<version>_linux_x86_64.rpm` |
| Any other Linux | `goxlr-hub_<version>_linux_x86_64.AppImage` |

The installers are built by GitHub Actions from the tagged source, never on
a personal computer.

### Windows

The installer is not code-signed yet. Windows shows "Windows protected your
PC": choose **More info**, then **Run anyway**. The app installs for the
current user and asks for no administrator rights.

#### Windows driver

GoXLR Hub talks to the GoXLR through the official TC-Helicon driver and
never replaces it. The installer tells you when it is missing. Get it from
the GoXLR page of [tc-helicon.com](https://www.tc-helicon.com/), under
Downloads, install it, then start GoXLR Hub again. Without the driver the
app opens in demo mode.

### Linux

The `.deb` and `.rpm` packages add a udev rule that lets you use the GoXLR
without being root; plug the GoXLR in again after installing. With the
AppImage, copy
[`70-goxlr-hub.rules`](crates/app/installer/70-goxlr-hub.rules) to
`/etc/udev/rules.d/` yourself.

### Verify a download

Every file of a release comes with a
[build provenance attestation](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations):
proof that it was built by this repository's release workflow. With the
[GitHub CLI](https://cli.github.com/):

```bash
gh attestation verify goxlr-hub_<version>_windows_x64-setup.exe --repo SilvunDev/goxlr-hub
```

`SHA256SUMS.txt`, next to the files, lists their checksums.

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
