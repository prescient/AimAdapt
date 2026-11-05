# Working on AimAdapt

Rules for anyone (people or coding agents) changing this repo.

## Every bug gets a regression test

When a bug is found, whether it's reported by a user, noticed in testing, or hit while playing, add a test that fails because of it before fixing it, then fix it.

- Put the test as close to the cause as possible: a unit test in the module that had the bug, or a fixture test for bad input files.
- Check that the test really catches the bug: it should fail on the old code and pass on the fixed code.
- Name or comment the test so it's clear which bug it guards against.
- If a bug can't be tested automatically (for example, it only shows in the game window), say so in the fix's commit message and add the closest test you can, such as one on the arguments, data or state that led to it.

[TESTING.md](TESTING.md) has the test layers and the fixture policy.

## Keep the README current

The README is how people learn what AimAdapt does, so it should always describe the app as it is now. Update it in the same change as every feature, fix or behavior change that affects what it says:

- New or changed features, scenarios, settings, ratings and commands.
- Removed features: take them out of the README too.
- Screenshots that no longer match the app: retake them from the bot or a test database (see Data and privacy).

Before committing, reread the README sections your change touches and check nothing in them is now wrong.

## Keep the user guide current

The app's Help page shows [the user guide](crates/aimadapt-app/ui/guide.md). It's written for players, in more detail than the README. Update it in the same change as anything a player would notice:

- A new or changed page, button, setting, scenario, workout, rating or key in the trainer: describe it where a player would look for it.
- A change that resets ratings or changes how something is scored: say so, in the section that explains it.
- Every user-visible change also gets a line in **What's new**, under the current month.
- On each release, bump the version line at the top ("This guide is for AimAdapt …") and check What's new covers the release.

`crates/aimadapt-app/tests/guide.rs` fails when the guide is missing a page, a built-in scenario or workout, a rating, a trainer key, a section a link points to, or the current version. Passing it isn't enough on its own: also reread the sections your change touches.

## Before committing

- Commit through the pre-commit hook (`git config core.hooksPath .githooks`). It runs formatting, clippy with warnings as errors, all tests, and a release build to `dist/AimAdapt.exe`. Don't skip it with `--no-verify`.
- Close AimAdapt first, or the hook can't replace `dist/AimAdapt.exe`.
- Keep [README.md](README.md) and the [user guide](crates/aimadapt-app/ui/guide.md) (see above), [SCENARIOS.md](SCENARIOS.md), [TESTING.md](TESTING.md) and [BACKLOG.md](BACKLOG.md) in step with what changed.

## Data and privacy

- The repo is public. Don't commit personal data: real databases, `dev-snapshot.json` exports, or screenshots of a real history.
- Use the bot (`--smoke`), a separate database (`--db`) and `--no-import` for test and demo data.
- Never edit a user's own KovaaK's files. Only create new files, with an `AimAdapt -` prefix.

## Content

- Scenario names and designs are original. Mechanics can follow common aim-trainer ideas, but don't copy other trainers' scenarios, names or assets.
