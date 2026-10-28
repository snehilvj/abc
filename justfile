# List recipes
default:
    @just --list

# Everything CI checks: formatting, clippy with warnings as errors, every test
# (snapshots must match, and no snapshot file may lack its test), and docs.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo insta test --workspace --check --unreferenced=reject
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

# Open a screen on fake data, e.g. `just gallery frame --state sidebar`
gallery *args:
    cargo run -q -p abc -- gallery {{args}}

# Record a screen's VHS tape to target/vhs/<screen>.gif, e.g. `just tape frame`
tape screen:
    cargo build -q -p abc
    mkdir -p target/vhs
    vhs docs/ui/tapes/{{screen}}.tape
