lint:
    cargo clippy --all --all-features -- -W clippy::pedantic

format:
    cargo fmt --all

fmt: format

test:
    cargo test --all-features

bench:
    cargo bench --all-features
