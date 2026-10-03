# Contributing to GoXLR Hub

Thanks for your interest. This document explains how changes move through the
project.

## Language

Code, comments, commits, issues, pull requests and documentation are written
in English.

## Workflow

1. Every change starts from an issue. Open one if none exists.
2. Create a branch from `main`: `<type>/<issue-number>-<short-description>`,
   for example `feat/12-fader-volume`.
3. Commit using [Conventional Commits](https://www.conventionalcommits.org/).
4. Open a pull request. Its title follows the same format, and its description
   links the issue with `Closes #<number>`.
5. All checks must pass before merging.
6. Pull requests are squash-merged, so `main` has one commit per change.

Nobody pushes directly to `main`.

## Commit format

```
<type>(<optional scope>): <summary in imperative mood>
```

| Type | Use for |
|---|---|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation only |
| `refactor` | Code change with no behaviour change |
| `perf` | Performance improvement |
| `test` | Adding or fixing tests |
| `build` | Build system or dependencies |
| `ci` | CI configuration |
| `chore` | Anything else that does not touch source or tests |

Add `!` after the type for a breaking change: `feat!: drop legacy profiles`.

Enable the local commit message check once after cloning:

```bash
git config core.hooksPath .githooks
```

## Checks

Every pull request runs formatting, linting, tests and a secret scan. Run them
locally before pushing:

```bash
pnpm check
pnpm test
pnpm build
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

## Versions and changelog

Versions follow [Semantic Versioning](https://semver.org/). Releases and
`CHANGELOG.md` are generated from commit history; do not edit the changelog by
hand.

## Reporting security issues

Do not open a public issue. See [SECURITY.md](SECURITY.md).
