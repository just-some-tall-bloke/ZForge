use zforge_core::{Cpu, FlatMemory, Memory, flags};
use zforge_decoder::DecodeError;
use zforge_interpreter::step;

#[test]
fn runs_countdown_and_accounts_for_both_branch_outcomes() {
    let mut cpu = Cpu::default();
    let mut memory = FlatMemory::default();
    memory
        .load(0, &[0x3e, 3, 0x3d, 0x20, 0xfd, 0x32, 0, 0x80, 0x76])
        .unwrap();
    memory.write(0x8000, 0xff);
    let mut trace = Vec::new();
    for _ in 0..9 {
        trace.push(step(&mut cpu, &mut memory).unwrap().unwrap());
    }
    assert_eq!(
        trace
            .iter()
            .map(|record| (record.pc, record.t_states))
            .collect::<Vec<_>>(),
        [
            (0, 7),
            (2, 4),
            (3, 12),
            (2, 4),
            (3, 12),
            (2, 4),
            (3, 7),
            (5, 13),
            (8, 4)
        ]
    );
    assert_eq!(cpu.main.a, 0);
    assert_eq!(memory.read(0x8000), 0);
    assert_eq!(cpu.main.f, flags::Z | flags::N);
    assert_eq!(cpu.t_states, 67);
    assert_eq!(cpu.pc, 9);
    assert_eq!(cpu.r, 9);
    assert!(cpu.halted);
    let halted = cpu.clone();
    assert_eq!(step(&mut cpu, &mut memory), Ok(None));
    assert_eq!(cpu, halted);
}

#[test]
fn register_and_memory_loads_preserve_flags_and_use_correct_timing() {
    let mut cpu = Cpu::default();
    cpu.main.f = flags::C;
    let mut memory = FlatMemory::default();
    memory
        .load(
            0,
            &[
                0x21, 0x34, 0x12, 0x36, 0x7f, 0x46, 0x48, 0x71, 0x3a, 0x34, 0x12,
            ],
        )
        .unwrap();
    for expected in [10, 10, 7, 4, 7, 13] {
        assert_eq!(
            step(&mut cpu, &mut memory).unwrap().unwrap().t_states,
            expected
        );
        assert_eq!(cpu.main.f, flags::C);
    }
    assert_eq!(cpu.main.bc, 0x7f7f);
    assert_eq!(cpu.main.a, 0x7f);
    assert_eq!(memory.read(0x1234), 0x7f);
}

#[test]
fn call_and_return_wrap_the_stack_and_use_little_endian_addresses() {
    let mut cpu = Cpu {
        pc: 0x1234,
        sp: 1,
        ..Cpu::default()
    };
    let mut memory = FlatMemory::default();
    memory.load(0x1234, &[0xcd, 0x00, 0x20]).unwrap();
    memory.write(0x2000, 0xc9);
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 17);
    assert_eq!((cpu.pc, cpu.sp), (0x2000, 0xffff));
    assert_eq!((memory.read(0xffff), memory.read(0)), (0x37, 0x12));
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 10);
    assert_eq!((cpu.pc, cpu.sp), (0x1237, 1));
}

#[test]
fn jumps_use_absolute_targets_and_signed_relative_offsets() {
    let mut cpu = Cpu::default();
    cpu.main.f = flags::C;
    let mut memory = FlatMemory::default();
    memory.load(0, &[0xc3, 0xfe, 0xff]).unwrap();
    memory.load(0xfffe, &[0x38, 0xfe]).unwrap();
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 10);
    assert_eq!(cpu.pc, 0xfffe);
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 12);
    assert_eq!(cpu.pc, 0xfffe);
    cpu.main.f = 0;
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 7);
    assert_eq!(cpu.pc, 0);
}

#[test]
fn arithmetic_sets_overflow_half_carry_and_borrow() {
    for (opcode, a, value, result, f) in [
        (0xc6, 0x7f, 1, 0x80, flags::S | flags::H | flags::PV),
        (0xc6, 0xff, 1, 0, flags::Z | flags::H | flags::C),
        (
            0xd6,
            0x80,
            1,
            0x7f,
            flags::X | flags::Y | flags::H | flags::PV | flags::N,
        ),
        (
            0xd6,
            0,
            1,
            0xff,
            flags::S | flags::X | flags::Y | flags::H | flags::N | flags::C,
        ),
        (
            0xfe,
            0,
            0x28,
            0,
            flags::S | flags::X | flags::Y | flags::H | flags::N | flags::C,
        ),
    ] {
        let mut memory = FlatMemory::default();
        memory.load(0, &[opcode, value]).unwrap();
        let mut cpu = Cpu::default();
        cpu.main.a = a;
        assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 7);
        assert_eq!((cpu.main.a, cpu.main.f), (result, f));
    }
}

#[test]
fn indirect_increment_and_decrement_preserve_carry() {
    let mut cpu = Cpu::default();
    cpu.main.hl = 0xffff;
    cpu.main.f = flags::C;
    let mut memory = FlatMemory::default();
    memory.load(0, &[0x34, 0x35]).unwrap();
    memory.write(0xffff, 0x7f);
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 11);
    assert_eq!(memory.read(0xffff), 0x80);
    assert_eq!(cpu.main.f, flags::S | flags::H | flags::PV | flags::C);
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 11);
    assert_eq!(memory.read(0xffff), 0x7f);
    assert_eq!(
        cpu.main.f,
        flags::X | flags::Y | flags::H | flags::PV | flags::N | flags::C
    );
}

#[test]
fn refresh_preserves_bit_seven_and_unsupported_opcode_preserves_cpu() {
    let mut cpu = Cpu {
        r: 0xff,
        ..Cpu::default()
    };
    let mut memory = FlatMemory::default();
    memory.load(0, &[0x00, 0xed]).unwrap();
    assert_eq!(step(&mut cpu, &mut memory).unwrap().unwrap().t_states, 4);
    assert_eq!(cpu.r, 0x80);
    let before = cpu.clone();
    assert_eq!(
        step(&mut cpu, &mut memory),
        Err(DecodeError {
            pc: 1,
            opcode: 0xed
        })
    );
    assert_eq!(cpu, before);
}
