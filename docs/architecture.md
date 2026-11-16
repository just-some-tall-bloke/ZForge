# ZForge architecture

| Crate | Responsibility |
| --- | --- |
| `zforge-core` | CPU registers and flags, a byte-oriented `Memory` trait, and a 64 KiB flat RAM implementation |
| `zforge-decoder` | Read opcode and operand bytes from guest memory and produce a typed instruction with its fall-through PC |
| `zforge-interpreter` | Execute decoded instructions, maintain CPU state and instruction timing, and return step records for traces |
| `zforge-native` | Lower a subset of decoded instructions to Cranelift IR and execute bounded x86-64 blocks |
| `zforge-cpm` | Load a transient program in flat RAM and dispatch a minimal set of CP/M services from native block exits |

The interpreter and native backend share the typed decoder. The interpreter, decoder, and core use only the standard library.

## Reference interpreter

Each successful step advances PC to the decoded fall-through address, updates the low seven bits of R for the opcode fetch, and executes the decoded instruction. Branches and calls can replace PC. The returned record includes the starting PC, decoded instruction, and elapsed T-states. The CPU keeps an accumulated T-state count.

Guest fetches, relative branches, and stack accesses wrap at 16 bits. Immediate words and stack values use little-endian order. Loading a host image requires a contiguous fit in RAM and checks its bounds before writing. Unsupported opcodes report the starting PC and opcode and leave CPU state unchanged. A custom memory implementation can still observe the attempted opcode read.

## Interpreter instructions

| Instructions | T-states |
| --- | --- |
| `NOP`, `HALT` | 4 |
| `LD r,r'`, `LD r,(HL)`, `LD (HL),r` | 4 between registers, 7 with memory |
| `LD r,n`, `LD (HL),n` | 7 and 10 |
| `LD BC,nn`, `LD DE,nn`, `LD HL,nn`, `LD SP,nn` | 10 |
| `LD A,(nn)`, `LD (nn),A` | 13 |
| `INC r`, `DEC r`, `INC (HL)`, `DEC (HL)` | 4 for registers, 11 for memory |
| `ADD A,n`, `SUB n`, `CP n` | 7 |
| `JP nn`, `CALL nn`, `RET` | 10, 17, and 10 |
| `JR e`, `JR cc,e` with NZ/Z/NC/C | 12 when taken, 7 when a condition fails |

Instruction timing and documented flag semantics follow the Z80 CPU user manual. For immediate `CP`, flag bits 3 and 5 come from the operand. [Zilog CPU user manual](https://www.zilog.com/docs/z80/um0080.pdf) · [Sean Young's Z80 documentation](https://z80.info/zip/z80-documented.pdf)

## Native execution

The proof of concept decodes the next block through an immutable view of flat RAM. The interpreter's execution fetches still use `Memory::read` so a future adapter can observe them. The native backend lowers the decoded instructions directly to Cranelift IR and keeps generated-code calls behind its own explicit ABI.

Blocks respect a caller-supplied instruction budget and return to the host after control flow, HALT, or a store. A store can change subsequent instructions in the same address range. The dispatcher applies the write before the next decode, which lets the next block see the new bytes. Translation is per invocation. The cache, mapping identity, and hardware scheduling belong to later milestones.

The CP/M demo starts its `.COM` image at `0x0100`. At `0x0005` the host handles BDOS function 9 using the dollar-terminated string at DE and resumes at the guest return address. BDOS function 0 and a jump to address 0 end the run. The interpreter's and backend's T-state counters describe guest instructions. Host service time is outside that count. [Digital Research CP/M 2.2 system interface](https://www.gaby.de/cpm/manuals/archive/cpm22htm/ch5.htm)

## Current limits

- CPU defaults give tests a deterministic initial state. Hardware reset behavior and undocumented internal state such as WZ are outside this model.
- The CPU stores the alternate and index registers and interrupt state. The current instruction subset leaves those fields unused.
- HALT advances the exposed PC to the following instruction and stops stepping. Repeated HALT fetches, interrupt entry, and resumption require a later timing model.
- Instruction timings are aggregate counts. Memory contention, wait states, I/O, and hardware events require machine integration.
- The decoder reports all prefixes and other unimplemented opcodes as unsupported. The native backend handles a smaller subset and targets x86-64.
- The CP/M demo models the console string and termination paths. File services, console input, command-line arguments, and general `.COM` compatibility are future work.
