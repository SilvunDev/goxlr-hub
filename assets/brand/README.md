# GoXLR Hub brand

The visual identity of GoXLR Hub. It is deliberately unrelated to TC-Helicon's
branding: no teal-on-black, no TC-Helicon or GoXLR logotype, no product
photography.

## Logo

The mark is an "H" lit on a dot-matrix screen, the kind the GoXLR has above
each fader. The wordmark is drawn with the same dots, so the logo needs no
font.

| File | Use |
| --- | --- |
| [`mark.svg`](mark.svg) | App icon, favicon, avatar. |
| [`logo-dark.svg`](logo-dark.svg) | Mark and wordmark, for dark backgrounds. |
| [`logo-light.svg`](logo-light.svg) | Mark and wordmark, for light backgrounds. |
| [`social-preview.png`](social-preview.png) | Repository social preview, 1280 × 640, a still from the teaser video. |
| [`installer-sidebar.svg`](installer-sidebar.svg) | Windows installer, welcome and finish pages, 164 × 314. |
| [`installer-header.svg`](installer-header.svg) | Windows installer and uninstaller, top of the other pages, 150 × 57. |

The two installer pictures are drawn from the logo by
[`scripts/installer-images.mjs`](../../scripts/installer-images.mjs), which
also writes the BMP files the installer embeds, in `crates/app/installer/`.

Rules:

- Keep clear space around the logo equal to one column of the mark's dots.
- Do not recolour, stretch, rotate or add effects.
- Below 24 px, use the mark alone.
- Wherever the logo introduces the project, keep the "unofficial, not
  affiliated with TC-Helicon" notice nearby.

## Colours

| Name | Hex | Role |
| --- | --- | --- |
| Case | `#20241F` | Main background. |
| Panel | `#292E28` | Raised surfaces. |
| Unlit | `#414A40` | Borders, tracks, unlit dots. |
| Legend | `#A3AB9C` | Secondary text. |
| Silkscreen | `#F1F3EA` | Main text. |
| Amber | `#FFB224` | Primary accent: active state, main action. |
| Deep amber | `#A86A00` | Amber on light backgrounds. |
| Mint | `#7FE3C0` | Secondary accent, used sparingly. |

Text on Amber uses `#2B1D00`. Amber and Mint never carry meaning alone: pair
them with a label, icon or shape.

## Typography

| Role | Typeface | License |
| --- | --- | --- |
| Interface and body text | [Schibsted Grotesk](https://fonts.google.com/specimen/Schibsted+Grotesk) | OFL-1.1 |
| Labels, values, code | [Martian Mono](https://fonts.google.com/specimen/Martian+Mono) | OFL-1.1 |

Labels in Martian Mono are uppercase with slight letter-spacing. The
dot-matrix lettering is reserved for the logo.
