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

https://github.com/user-attachments/assets/c5cfef23-aada-4469-9b5f-96104ea7d0e5

## Status

Early development. Nothing is usable yet.

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

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Brand

Logo, colours and typefaces are described in
[assets/brand](assets/brand/README.md).

## Credits

Device protocol knowledge comes from
[GoXLR Utility](https://github.com/GoXLR-on-Linux/goxlr-utility) (MIT).

## License

GoXLR Hub is licensed under the
[GNU General Public License v3.0 or later](LICENSE).

Plugins that only talk to GoXLR Hub through its public plugin interface are
separate works and may use any license.
