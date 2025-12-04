#!/bin/bash

set -euxo pipefail

cargo build
cargo test

cargo build -p proto-hal-build --all-features
cargo build -p proto-hal-model --all-features

TARGET="thumbv7em-none-eabihf"

rustup target add "$TARGET"
cargo build -p g4 --target "$TARGET"

cargo clippy -- --deny warnings
cargo clippy -p proto-hal-build --all-features -- --deny warnings
cargo clippy -p proto-hal-model --all-features -- --deny warnings
