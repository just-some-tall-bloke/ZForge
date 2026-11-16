# Contributing to ZForge

Use stable Rust with the `rustfmt` and `clippy` components installed. Run the same checks as GitHub Actions before submitting changes.

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --release
```

For changes to CPU behavior, include the opcode bytes, expected registers and flags, and T-state counts in a focused test. Record the source and licence for any external fixtures. The current tests and countdown example use original, inline machine-code fixtures.
