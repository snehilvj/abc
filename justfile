# List recipes
default:
    @just --list

# Everything CI checks: formatting, clippy with warnings as errors, every test
# (snapshots must match, and no snapshot file may lack its test), and docs.
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo insta test --workspace --all-features --check --unreferenced=reject
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked

# Open a screen on fake data, e.g. `just gallery frame --state sidebar`
gallery *args:
    cargo run -q -p abc -- gallery {{args}}

# Record a screen's VHS tape to target/vhs/<screen>.gif, e.g. `just tape frame`
tape screen:
    cargo build -q -p abc
    mkdir -p target/vhs
    vhs docs/ui/tapes/{{screen}}.tape

# Run this checkout's abc in the dev profile (shares nothing with stable), e.g. `just dev repo list`
dev *args:
    cargo build -q -p abc -p abcd
    target/debug/abc --profile dev {{args}}

# Make fixture repositories in the dev profile, one deleted to show as missing; again for a fresh set
fixtures:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build -q -p abc -p abcd
    dir="${XDG_STATE_HOME:-$HOME/.local/state}/abc/dev/fixtures"
    cargo run -q -p abc-core --features fixtures --example fixtures -- "$dir" |
        while read -r name path; do
            # Run again, they're already added: say nothing of it.
            target/debug/abc --profile dev repo add "$path" --name "$name" 2>&1 |
                grep -v "is already added" || true
        done
    rm -rf "$dir/old-tool"
    target/debug/abc --profile dev repo list

# Install abc and abcd side by side in ~/.cargo/bin, for the stable profile
install:
    cargo install --locked --path crates/abc
    cargo install --locked --path crates/abcd
