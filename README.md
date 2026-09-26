# pglite-sidecar

A small Rust CLI that embeds [PostgreSQL](https://www.postgresql.org/) via
[`pglite-rs`](https://crates.io/crates/pglite-rs) and exposes it on a **Unix
socket**. Any normal Postgres client — Go [`pgx`](https://github.com/jackc/pgx),
SQLx, `lib/pq`, `psql` — can connect with a standard connection URI.

No Docker, no system Postgres install, no TCP listener. The sidecar process
*is* the database.

## Why

You want a real Postgres for local tools, tests, or a same-host sidecar, but
you do not want to run a full server. `pglite-rs` links a patched Postgres 17
into a Rust binary and (with the `socket` + `multiple-process` features) serves
the wire protocol on a Unix socket so unmodified drivers work — including
connection pools.

This repo is that sidecar plus a minimal Go example.

## Prerequisites

| Tool | Notes |
| --- | --- |
| **Rust 1.87+** | `pglite-rs` 0.2.7 lists 1.85, but its `ruzstd` build dependency needs 1.87. This repo uses `rust-toolchain.toml` (`stable`) and `rust-version = "1.87"`. |
| **Go 1.22+** | Only for `examples/go`. |
| **Linux or macOS** | The socket gateway is Unix-only. **Windows is unsupported.** |
| **C toolchain** | `gcc`/`clang` and a linker. First `cargo build` downloads a native Postgres runtime and may take a while. |

No Postgres packages, no Docker, and no secrets are required for the happy path.

If `cargo build` fails, typical causes are:

- Rust older than 1.87 (`rustup toolchain install stable`)
- Missing C compiler / linker
- An unsupported target (the crate ships prebuilt runtimes for common Unix triples)

## Build

```bash
cargo build
# or, for a smaller/faster binary:
cargo build --release
```

The binary is `target/debug/pglite-sidecar` (or `target/release/pglite-sidecar`).

The Go example:

```bash
cd examples/go
go build -o pglite-go-example .
```

## Run

### Two terminals (recommended)

**Terminal 1** — start the sidecar. The **first line of stdout** is the
connection URI; everything else goes to stderr.

```bash
cargo run -- --data-dir ./data
# or:
./target/debug/pglite-sidecar -d ./data --extra-connections 8
```

Example stdout:

```
postgresql://postgres@localhost/postgres?host=/tmp/pgl-12345-0
```

Leave this process running. Ctrl-C (or SIGTERM) closes the engine cleanly.

**Terminal 2** — point a client at that URI:

```bash
export DATABASE_URL='postgresql://postgres@localhost/postgres?host=/tmp/pgl-12345-0'
cd examples/go
go run .
```

Expected:

```
ok: connected via pgx, SELECT 1 = 1, latest greeting = "hello from go"
```

`psql` works the same way:

```bash
psql "$DATABASE_URL" -c 'SELECT version();'
```

### One-shot demo script

[`scripts/demo.sh`](scripts/demo.sh) builds the sidecar, waits for the URI, runs
the Go example, then sends SIGTERM:

```bash
./scripts/demo.sh
```

Override the data directory with `PGLITE_DATA_DIR` if you want.

## Connection string

`pglite-rs` prints a libpq-style URI:

```
postgresql://<user>@localhost/<database>?host=<socket-dir>
```

Defaults (from `MultiProcessOptions::default()`):

- user: `postgres`
- database: `postgres`
- host: a short directory (often `/tmp` or `/dev/shm` on Linux) containing `.s.PGSQL.5432`

The `host` query parameter is a **directory**, not the socket file itself — the
same convention as libpq / `pgx`. There is no password. There is no TCP port:
clients must speak Unix sockets on the same machine.

Scripts should read **only the first line of stdout** and treat stderr as logs.

## CLI

```
pglite-sidecar [OPTIONS]

Options:
  -d, --data-dir <DIR>           PostgreSQL data directory [default: ./data]
      --extra-connections <N>    Slots reserved for external clients [default: 4]
  -h, --help
  -V, --version
```

`--extra-connections` maps to `MultiProcessOptions.extra_connections`. Size it
to your client pool (`pgxpool`, SQLx, …). The sidecar itself also keeps an
internal pool (`max_connections`, default 5).

On start the sidecar:

1. Opens the data dir with `PGlite::open_multi_process`
2. Calls `unix_uri()` (first call starts / returns the gateway)
3. Prints the URI on stdout and flushes
4. Blocks until SIGINT or SIGTERM
5. Calls `close()` and exits

Keep the `PGlite` instance alive for the whole process lifetime. Connect
clients after the URI is printed; disconnect them before the sidecar exits.

## Limitations

- **Unix socket only.** No TCP. Clients must run on the same host (and typically
  the same mount namespace) as the sidecar.
- **Windows is unsupported.** `pglite-rs`’s `multiple-process` / socket gateway
  is Unix-only; this binary fails to compile on Windows.
- **The binary is not tiny.** The crate embeds a patched Postgres 17 runtime
  (`libpglite` + extracted `bin/postgres` for multi-process). Expect a large
  compile artifact and a first-run extract of the runtime.
- **Crate maturity.** `pglite-rs` is young (0.2.x). APIs, features, and
  supported targets can change. This sidecar pins `0.2.7`.
- **Same-process reopen.** After `close()`, `pglite-rs` cannot reopen in the
  same process — restart the sidecar instead.
- **SIGKILL orphans.** If the sidecar is `SIGKILL`ed, a child postmaster may
  linger until the next open (Postgres stale `postmaster.pid` handling) and
  socket dirs under `/tmp` may remain until the OS cleans them.
- **Not a production multi-tenant server.** Fine for local apps, tests, and
  same-host sidecars. Do not expose the socket across users or hosts you do
  not trust.

## How it works

```
  Go / pgx / psql                 pglite-sidecar
        │                                │
        │  postgresql://…?host=/tmp/pgl-…│
        └──────── Unix socket ──────────►│
                                         │  PGlite::open_multi_process
                                         │  child postmaster + extra_connections
                                         ▼
                                    embedded Postgres 17
```

This project uses:

- `pglite-rs` features `socket` (default) and `multiple-process`
- `PGlite::open_multi_process(data_dir, MultiProcessOptions { extra_connections, .. })`
- `unix_uri()` then `close()` on shutdown

## Credits

- [pglite-rs](https://github.com/Midwess/pglite-rs) — in-process Postgres for Rust
  (crate import name `pglite`)
- [postgres-pglite](https://github.com/electric-sql/postgres-pglite) and
  [Electric](https://electric-sql.com/) — the embedded Postgres engine
- [pgx](https://github.com/jackc/pgx) — Go Postgres driver used in the example

## License

[MIT](LICENSE)
