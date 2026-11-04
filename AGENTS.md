# Working on RustyAim

Rules for anyone (people or coding agents) changing this repo.

## Every bug gets a regression test

When a bug is found, whether it's reported by a user, noticed in testing, or hit while playing, add a test that fails because of it before fixing it, then fix it.

- Put the test as close to the cause as possible: a unit test in the module that had the bug, or a fixture test for bad input files.
- Check that the test really catches the bug: it should fail on the old code and pass on the fixed code.
- Name or comment the test so it's clear which bug it guards against.
- If a bug can't be tested automatically (for example, it only shows in the game window), say so in the fix's commit message and add the closest test you can, such as one on the arguments, data or state that led to it.

[TESTING.md](TESTING.md) has the test layers and the fixture policy.

## Before committing

- Commit through the pre-commit hook (`git config core.hooksPath .githooks`). It runs formatting, clippy with warnings as errors, all tests, and a release build to `dist/RustyAim.exe`. Don't skip it with `--no-verify`.
- Close RustyAim first, or the hook can't replace `dist/RustyAim.exe`.
- Keep [README.md](README.md), [SCENARIOS.md](SCENARIOS.md), [TESTING.md](TESTING.md) and [BACKLOG.md](BACKLOG.md) in step with what changed.

## Data and privacy

- The repo is public. Don't commit personal data: real databases, `dev-snapshot.json` exports, or screenshots of a real history.
- Use the bot (`--smoke`), a separate database (`--db`) and `--no-import` for test and demo data.
- Never edit a user's own KovaaK's files. Only create new files, with a `RustyAim -` prefix.

## Content

- Scenario names and designs are original. Mechanics can follow common aim-trainer ideas, but don't copy other trainers' scenarios, names or assets.
