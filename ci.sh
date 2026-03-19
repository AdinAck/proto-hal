#!/bin/bash

set -euxo pipefail

cargo build --all-features
cargo clippy -- --deny warnings
cargo test

TARGET="thumbv7em-none-eabihf"

rustup target add "$TARGET"
cargo build -p g4 --target "$TARGET"
