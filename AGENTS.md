# ZForge development

- Follow the current milestone in README.md. Keep the CPU and decoder independent of machine adapters and native backends.
- Use stable Rust, the workspace lints, and explicit wrapping arithmetic for guest addresses and values.
- Keep the decoder as the shared source of opcode and operand interpretation. Return a diagnostic for unsupported opcodes.
- Cover instruction results, flags, and T-states with focused tests. Add regression tests for confirmed CPU and translation bugs.
- Validate instruction semantics against independent test vectors. Compare native execution against the interpreter at block exits.
- Decode for translation through side-effect-free code inspection. Keep generated-code calls and their ABI inside the native backend.
- Keep guest fixtures original or document their source and licence. Do not bundle proprietary binaries.
- Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release`.
- Use feature branches and small commits. Document supported behavior and known gaps as they change.
