# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Redis server clone (Coding Challenges). Tokio-based async TCP server on `127.0.0.1:6379` speaking the RESP protocol. See `JOURNAL.md` for design history and rationale.

## Commands

Always run cargo in release mode.

- Build: `cargo build --release`
- Run server: `cargo run --release` (set `RUST_LOG=trace|warn|error` for logs)
- Test all: `cargo test --release`
- Test single: `cargo test --release <name>` (e.g. `cargo test --release execute_lpush_ok`)
- Test one module: `cargo test --release cmd::execution::list`
- Lint: `cargo clippy --release`
- Bench: `cargo bench`

## Architecture

Request flow: TCP bytes -> `Deserializer` (RESP parsing) -> `Vec<String>` -> `Request::try_from` -> `Request::execute(&Db)` -> `Response::serialize` -> bytes.

`main.rs` owns the listener and a background tokio task that runs `db::remove_expired_entries` on a random sample (active expiration). Passive expiration happens inside command execution: each command checks `Object::is_expired()` and removes the entry before acting.

`Db = Arc<Mutex<IndexMap<String, Object>>>`. `IndexMap` is required so the active-expiration sampler can index randomly. `Object { value: Value, expiration: Option<SystemTime> }` where `Value` is an enum of `Integer(i64) | String(String) | List(VecDeque<String>)`. Mutex is `std::sync::Mutex` (not tokio's) since it is never held across `await`.

### Command module layout

Commands are grouped by family. Each family has a parser and an execution module:

- `cmd/types.rs`: lowercase command-name string constants
- `cmd/request.rs`: `Request` enum, `TryFrom<Vec<String>>` dispatch, and `execute(&Db) -> Response`
- `cmd/parser/<family>.rs`: parses `&[String]` args into a typed struct (e.g. `arithmetic::Integer`, `list::List`, `set::Set`)
- `cmd/execution/<family>.rs`: enum variant per command in the family + a shared `execute` that takes the parsed inputs; uses an `operation` method returning a closure (`Box<dyn Fn ...>`) selected per variant

To add a new command in an existing family: add the constant in `types.rs`, add the variant in the execution enum (and its closure in `operation`), wire a `Request` variant + `try_from` arm + `execute` arm. Tests live in `#[cfg(test)] mod tests` inside each touched file.

### Known shape quirks

- `arithmetic::Integer::parse` and `list::List::parse` hardcode their command name (`INCRBY` / `LPUSH`) in the `WrongNumberOfArguments` error, so error messages can be misleading when called for sibling commands (`DECRBY`, `RPUSH`). Parameterizing this is a tracked TODO in `JOURNAL.md`.
- `Request::try_from` and the family parsers both validate arity. The request-level check covers the "no key" case, the parser covers "no values".
