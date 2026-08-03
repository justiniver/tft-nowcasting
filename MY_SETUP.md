# My Rust Setup

## Local tools

- macOS on Apple Silicon (`arm64`)
- Shell: zsh
- Rust toolchain: stable (`aarch64-apple-darwin`)
- `rustc 1.97.1` — compiles Rust code
- `cargo 1.97.1` — builds, runs, tests, and manages dependencies
- `rustup 1.29.0` — installs and switches Rust toolchains

Rust was installed with rustup. Its commands live in `~/.cargo/bin`, and my zsh setup loads them from `~/.cargo/env`.

## This project

- Rust edition: `2024`
- Application entry point: `src/main.rs`
- Project settings and dependencies: `Cargo.toml`
- Exact dependency versions: `Cargo.lock`
- No external dependencies yet

Cargo puts generated executables and build files under `target/`. That directory is ignored by Git, so it should not be committed.

## Commands I will probably use

```bash
cargo run              # Build and run the app
cargo check            # Check the code without producing a final executable
cargo test             # Run tests
cargo build            # Make a development build
cargo build --release  # Make an optimized build
cargo clean            # Delete generated files under target/
cargo fmt              # Format the Rust code
cargo clippy           # Check for common mistakes and improvements
```

Development builds go in `target/debug/`. Optimized builds go in `target/release/`.
