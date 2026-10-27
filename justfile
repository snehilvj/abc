# List recipes
default:
    @just --list

# Format check, clippy with warnings as errors, and every test
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
