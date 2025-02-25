lint:
    cargo clippy --all --fix --all-features --allow-dirty --allow-staged -- -W clippy::pedantic

format:
    cargo fmt --all

fmt: format

test:
    cargo test --all-features
