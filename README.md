# ZForge

**Old Code. New Silicon.**

ZForge is a Z80-to-native binary translation project for software originally written for Z80-based computers, consoles, and embedded systems. The goal is to share a decoder and CPU semantics across a reference interpreter and native execution on x86-64 and AArch64.

## Milestone 1

The Rust workspace includes a CPU state model, 64 KiB of flat RAM, an unprefixed instruction decoder, and a reference interpreter with instruction-level T-state accounting. Each successful step returns a record callers can use for tracing. The included countdown program runs a conditional loop, stores its result to RAM, and halts.

With stable Rust installed, run:

```sh
cargo run -p zforge-interpreter --example countdown
```

The example ends with `RAM[0x8000] = 0, total T-states = 67`.

See [architecture and supported instructions](docs/architecture.md) and [contributing](CONTRIBUTING.md).

## Roadmap

1. **Foundation** — execute small programs in the reference interpreter.
2. **Native proof of concept** — define the initial IR, evaluate Cranelift, and compare generated x86-64 code against the interpreter.
3. **JIT** — cache translated blocks and handle indirect control flow, mapping changes, and self-modifying code.
4. **CP/M runtime** — run `.COM` programs with a machine adapter and a minimal BDOS.
5. **AOT** — generate and link native executables with explicit runtime fallback where needed.
6. **Hybrid** — combine precompiled and JIT blocks.
7. **ZX Spectrum 48K** — load snapshots and model hardware and timing.
8. **AArch64** — add a native backend and differential coverage.
9. **Expansion** — add machines, improve timing accuracy, and extend tooling.

## Licence

Available under either the [MIT licence](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE).
