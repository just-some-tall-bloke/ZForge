# ZForge

**Old Code. New Silicon.**

ZForge is a Z80-to-native binary translation project for software originally written for Z80-based computers, consoles, and embedded systems. The goal is to share a decoder and CPU semantics across a reference interpreter and native execution on x86-64 and AArch64.

## Milestone 1

The reference interpreter models the CPU state and 64 KiB of flat RAM. It executes a subset of unprefixed opcodes, counts T-states, and returns a trace record for each instruction. The included countdown program runs a conditional loop, stores its result to RAM, and halts.

With stable Rust installed, run:

```sh
cargo run -p zforge-interpreter --example countdown
```

The example ends with `RAM[0x8000] = 0, total T-states = 67`.

## Milestone 2 native proof of concept

On x86-64, Cranelift compiles a small instruction subset into native blocks. Tests compare the native countdown against the interpreter after each block. A second demo loads an original CP/M `.COM` image at `0x0100`, prints through a host BDOS handler, and exits.

```sh
cargo run -p zforge-native --example countdown
cargo run -p zforge-cpm --example print-and-exit
```

See [architecture and supported instructions](docs/architecture.md) and [contributing](CONTRIBUTING.md).

## Roadmap

1. **Foundation** — execute small programs in the reference interpreter.
2. **Native proof of concept** — check the interpreter against independent vectors, compare Cranelift blocks against it, and run a minimal CP/M print-and-exit program.
3. **JIT** — cache translated blocks and handle indirect control flow, mapping changes, and self-modifying code.
4. **CP/M runtime** — expand `.COM` compatibility and add command-line and file services.
5. **AOT** — generate and link native executables with explicit runtime fallback where needed.
6. **Hybrid** — combine precompiled and JIT blocks.
7. **ZX Spectrum 48K** — load snapshots and model hardware and timing.
8. **AArch64** — add a native backend and differential coverage.
9. **Expansion** — add machines, improve timing accuracy, and extend tooling.

## Licence

Available under either the [MIT licence](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE).
