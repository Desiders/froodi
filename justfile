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
    cargo test --no-default-features --features ruststream
    cargo test --no-default-features --features ruststream,async

test: test-basic test-default test-all-features test-async test-integrations

test-compilation:
    cargo test -p froodi --features async --test compile_fail --test validation --test downstream

overwrite-ui-tests:
    TRYBUILD=overwrite cargo test -p froodi --features async --test compile_fail registration_errors

bench-compilation:
    rustc --edition=2021 tools/compile_bench.rs -o /tmp/froodi-build-bench
    /tmp/froodi-build-bench "$PWD" /tmp/froodi-build-measurements 3 clean-app,topology default chain100,flat500

bench-init:
    cargo bench -p froodi --profile release --frozen --bench sync_container_init
    cargo bench -p froodi --profile release --frozen --bench async_container_init --features async

bench-resolve:
    cargo bench -p froodi --profile release --frozen --bench container_resolve --no-default-features
    cargo bench -p froodi --profile release --frozen --bench async_container_resolve --no-default-features --features async

bench-resolve-concurrent:
    cargo bench -p froodi --profile release --frozen --bench container_resolve_concurrent --no-default-features --features thread_safe,std
    cargo bench -p froodi --profile release --frozen --bench async_container_resolve_concurrent --no-default-features --features async,thread_safe

bench-registry-lifecycle:
    cargo bench -p froodi --profile release --frozen --bench registry_lifecycle --features async

bench-fragments:
    cargo bench -p froodi --profile release --frozen --bench fragment_compare --features async

bench: bench-init bench-resolve bench-resolve-concurrent bench-registry-lifecycle bench-fragments
