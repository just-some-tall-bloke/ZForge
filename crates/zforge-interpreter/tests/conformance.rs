use zforge_core::{Cpu, FlatMemory, InterruptMode, Memory, Registers};
use zforge_interpreter::step;

const VECTORS: &str = include_str!("fixtures/single-step/vectors.tsv");

#[test]
fn matches_independent_single_step_vectors() {
    for line in VECTORS.lines().skip(1) {
        let columns: Vec<_> = line.split('\t').collect();
        let [
            name,
            initial_cpu,
            initial_ram,
            final_cpu,
            final_ram,
            t_states,
        ] = columns[..]
        else {
            panic!("invalid fixture row: {line}");
        };

        let mut cpu = parse_cpu(initial_cpu);
        let mut memory = FlatMemory::default();
        load_ram(&mut memory, initial_ram);
        let mut expected_memory = memory.clone();
        load_ram(&mut expected_memory, final_ram);

        let mut expected_cpu = parse_cpu(final_cpu);
        expected_cpu.t_states = t_states.parse().unwrap();
        expected_cpu.halted = name.starts_with("76 ");

        let record = step(&mut cpu, &mut memory)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .unwrap_or_else(|| panic!("{name}: no instruction executed"));
        assert_eq!(u64::from(record.t_states), expected_cpu.t_states, "{name}");
        assert_eq!(cpu, expected_cpu, "{name}");

        for address in u16::MIN..=u16::MAX {
            assert_eq!(
                memory.read(address),
                expected_memory.read(address),
                "{name}: RAM at {address:#06x}"
            );
        }
    }
}

fn parse_cpu(state: &str) -> Cpu {
    let words: Vec<_> = state.split_ascii_whitespace().map(hex).collect();
    let [
        pc,
        sp,
        af,
        bc,
        de,
        hl,
        ir,
        ix,
        iy,
        af_alt,
        bc_alt,
        de_alt,
        hl_alt,
        im,
        iff1,
        iff2,
    ] = words[..]
    else {
        panic!("invalid fixture CPU state: {state}");
    };
    let [i, r] = ir.to_be_bytes();
    Cpu {
        main: register_bank(af, bc, de, hl),
        alternate: register_bank(af_alt, bc_alt, de_alt, hl_alt),
        ix,
        iy,
        sp,
        pc,
        i,
        r,
        iff1: iff1 != 0,
        iff2: iff2 != 0,
        interrupt_mode: match im {
            0 => InterruptMode::Mode0,
            1 => InterruptMode::Mode1,
            2 => InterruptMode::Mode2,
            _ => panic!("invalid fixture interrupt mode: {im}"),
        },
        ..Cpu::default()
    }
}

fn register_bank(af: u16, bc: u16, de: u16, hl: u16) -> Registers {
    let [a, f] = af.to_be_bytes();
    Registers { a, f, bc, de, hl }
}

fn load_ram(memory: &mut FlatMemory, bytes: &str) {
    for entry in bytes.split_ascii_whitespace() {
        let (address, value) = entry.split_once(':').unwrap();
        memory.write(hex(address), u8::try_from(hex(value)).unwrap());
    }
}

fn hex(value: &str) -> u16 {
    u16::from_str_radix(value, 16).unwrap()
}
