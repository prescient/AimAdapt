# RustyAim

Tracks your KovaaK's aim training: imports the per-run stats files into a local SQLite database, then reports progress and training time. See [BACKLOG.md](BACKLOG.md) for the plan.

## Layout

- `crates/rustyaim-core` handles parsing, storage, import and summaries. All logic lives here.
- `crates/rustyaim-cli` is the `rustyaim-cli` command line front end.


## Usage

```bash
cargo run -p rustyaim-cli -- import
```

```bash
cargo run -p rustyaim-cli -- summary
```

```bash
cargo run -p rustyaim-cli -- time
```

```bash
cargo run -p rustyaim-cli -- watch
```

`time` shows training time: active vs. session time, sessions, days, streak, weekday averages and runs by hour. Options: `--gap 15` (break in minutes that starts a new session), `--day-start 4` (hour a training day begins), `--goal 30` (daily goal in minutes).

The stats folder is found in your Steam libraries automatically; override it with `--stats <dir>`. The database defaults to `%APPDATA%\RustyAim\rustyaim.db`; override it with `--db <file>`. `errors` lists files that couldn't be imported.

## Tests

```bash
cargo test --workspace
```
