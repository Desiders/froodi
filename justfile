lint:
    cargo clippy --all --all-features -- -W clippy::pedantic

format:
    cargo fmt --all

fmt: format

test-basic:
    cargo test --no-default-features

test-default:
    cargo test

test-all-features:
    cargo test --all-features

test-async:
    cargo test --no-default-features --features async
    cargo test --no-default-features --features async,thread_safe

test-integrations:
    cargo test --no-default-features --features axum
    cargo test --no-default-features --features axum,http2-axum

    cargo test --no-default-features --features dptree
    
    cargo test --no-default-features --features telers

test: test-basic test-default test-all-features test-async test-integrations

bench-init:
    cargo bench --profile release --frozen --bench sync_container_init
    cargo bench --profile release --frozen --bench async_container_init --features async

bench-resolve:
    cargo bench --profile release --frozen --bench container_resolve --no-default-features
    cargo bench --profile release --frozen --bench async_container_resolve --no-default-features --features async

bench-resolve-concurrent:
    cargo bench --profile release --frozen --bench container_resolve_concurrent --no-default-features --features thread_safe
    cargo bench --profile release --frozen --bench async_container_resolve_concurrent --no-default-features --features async,thread_safe

bench: bench-init bench-resolve
