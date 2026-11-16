# Z80 single-instruction fixtures

These 115 cases cover 28 unprefixed opcodes supported by ZForge. The inputs and
expected results come from the `v1` JSON files in [SingleStepTests/z80 at
`ebe1875d48f374bcfd4b505d8eb8ee751568b5f7`](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7/v1).
The upstream project derived them from an emulator based on the Ares Z80 core.
The accompanying [MIT licence](LICENSE) covers the imported data.

For each selected opcode, the table retains the first four cases, followed by
the first case for any cycle count missing from those four. This covers both
outcomes of each supported conditional `JR`. Each name matches the original
record. T-states are the decimal count of entries in the original `cycles`
array.

Columns are tab-separated. Within each CPU column, hexadecimal fields appear
in this order.

```text
PC SP AF BC DE HL IR IX IY AF' BC' DE' HL' IM IFF1 IFF2
```

RAM fields list hexadecimal `address:value` pairs and unspecified initial
RAM starts at zero. The final RAM pairs overlay the initial image. The test
checks the whole resulting image as well as the register state and T-states.
ZForge currently leaves `ei_delay` inactive and omits the upstream internal
`WZ`, `P`, and `Q` fields and per-cycle bus pins. The test also checks the
ZForge `halted` bit against the opcode because the fixtures omit that bit.
