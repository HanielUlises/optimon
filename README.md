# optimon

A small concurrent monitoring daemon for NVIDIA Optimus laptops on Ubuntu.

optimon runs independent [Tokio](https://tokio.rs/) tasks that poll GPU and
display state and emit structured [`tracing`](https://docs.rs/tracing) logs
**only when something meaningful changes** — a PRIME GPU switch, a significant
temperature or utilization move, or a monitor being connected or disconnected.
Steady state is silent.

> **Status:** v0.1 — solid monitoring and logging. Automated responses (e.g.
> re-running `autorandr` on display changes) are planned; see
> [`src/action.rs`](src/action.rs).

![optimon running in a terminal](assets/demo.png)

*Illustrative output — the startup lines are a real capture; the telemetry
spike, monitor-connect, and GPU-switch lines show what change events look like.*

## What it watches

| Watcher | Source | Logs when |
| --- | --- | --- |
| Active GPU | `prime-select query` | the system switches between `intel`, `nvidia`, and `on-demand` |
| NVIDIA telemetry | NVML (falls back to `nvidia-smi`) | temperature or utilization moves past a configurable threshold |
| Displays | `xrandr --query` | a monitor is connected, disconnected, or changes resolution |

Each watcher keeps a private in-memory snapshot of the last observed state, so
it only logs on an actual change rather than on every poll.

## Requirements

- Rust 2021 (stable)
- Ubuntu 22.04+ with the NVIDIA proprietary driver and Optimus/PRIME
- `prime-select` and `xrandr` on `PATH`
- An X11 session for display detection (`xrandr`)

NVML is used when available; if it can't initialize, telemetry falls back to
shelling out to `nvidia-smi`. If the dGPU is powered down under PRIME, telemetry
is simply skipped until it returns.

## Build & run

```sh
cargo build --release
./target/release/optimon
```

Or during development:

```sh
cargo run
```

Stop it with `Ctrl+C` — watchers are signalled to stop and joined before exit.

## Configuration

optimon reads `config.toml` from the working directory. Every field has a
default, so a missing or partial file is fine. Override the path with
`OPTIMON_CONFIG`.

```toml
[general]
# Log verbosity: "trace", "debug", "info", "warn", "error".
log_level = "info"

[gpu]
poll_interval_secs = 4   # how often to poll GPU state
temp_threshold_c   = 3   # min °C delta before logging a telemetry change
util_threshold_pct = 10  # min % delta before logging a telemetry change

[display]
poll_interval_secs = 4   # how often to poll connected displays
```

The `RUST_LOG` environment variable, if set, takes precedence over
`log_level`:

```sh
RUST_LOG=optimon=debug cargo run
OPTIMON_CONFIG=/etc/optimon/config.toml ./target/release/optimon
```

## Project layout

```
optimon/
├── Cargo.toml
├── config.toml
└── src/
    ├── main.rs      # config load, tracing init, spawn watchers, wait for Ctrl+C
    ├── config.rs    # TOML config with defaults
    ├── error.rs     # crate error type (thiserror)
    ├── gpu.rs       # prime-select + NVML/nvidia-smi telemetry
    ├── display.rs   # xrandr parsing (+ unit tests)
    ├── watcher.rs   # independent Tokio watcher tasks
    └── action.rs    # placeholder for automated responses (v0.2+)
```

## Development

```sh
cargo test      # unit tests (xrandr parsing)
cargo clippy    # lints — clean
```

## License

MIT
