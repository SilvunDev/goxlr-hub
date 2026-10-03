# GoXLR Hub — rules for Claude Code

## Project

Open source, plugin-based control app for the TC-Helicon GoXLR.
The design spec and plans live in the private `goxlr-hub-internal` repository,
checked out next to this one at `../goxlr-hub-internal`.

## Language

- Everything in this repository is in English: code, comments, commits, pull
  requests, issues, documentation, user-facing strings.
- Working documents in other languages belong in `goxlr-hub-internal`, never
  here.

## Workflow

- One issue per task, attached to the milestone of its stage.
- One branch per change: `<type>/<issue-number>-<description>`. Never commit
  directly to `main`.
- One pull request per change, Conventional Commits title, `Closes #<number>`
  in the description, squash-merged.
- Commits follow Conventional Commits, in English, imperative mood.
- The changelog is generated, never edited by hand.

## Constraints

- Full-size GoXLR only. Windows and Linux.
- The official TC-Helicon driver stays on Windows. No custom driver.
- Never change a Windows security setting on an end user's machine.
- The project name stays "GoXLR Hub" with the "unofficial, not affiliated with
  TC-Helicon" notice.
- Attribution uses the global git identity. Never write a personal name.
- License: GPL-3.0-or-later. Do not copy code from projects with an
  incompatible license.
