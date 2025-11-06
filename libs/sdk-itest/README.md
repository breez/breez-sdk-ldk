# Breez SDK Integration Tests

Run `make build-images` before executing the test suite to ensure the necessary Docker images exist.

## Running the tests

Run the integration suite with `make test`.

The `test` target wraps `cargo test` and raises the stack size (`RUST_MIN_STACK=16777216`)
so the suite can exercise deep recursion without crashing.
