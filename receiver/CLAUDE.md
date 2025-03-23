# CXD5602PWBIMU Receiver - Guidelines

## Build Commands
- Build: `cargo build`
- Build (release): `cargo build --release`
- Run: `cargo run -- -p <PORT>`
- Check: `cargo check`
- Lint: `cargo clippy`
- Format: `cargo fmt`
- Test: `cargo test`
- Test single: `cargo test <TEST_NAME>`

## Code Style
- Use Rust 2024 edition conventions
- Error handling: Use `anyhow` for application errors and `thiserror` for library errors
- Follow standard Rust naming conventions (snake_case for variables/functions, CamelCase for types)
- Organize imports by standard library first, then external crates
- Use explicit error handling with proper error propagation (?, with_context)
- Prefer strong typing and struct-based organization over primitive types
- Document public API with rustdoc comments
- Format code with rustfmt (default settings)
- Use Result/Option for fallible operations instead of unwrap/expect