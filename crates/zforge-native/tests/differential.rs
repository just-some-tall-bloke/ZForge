#![cfg(target_arch = "x86_64")]

use zforge_core::{Cpu, FlatMemory, flags};
use zforge_interpreter::step;
use zforge_native::{NativeEngine, NativeError};

fn compare_block(
    engine: &mut NativeEngine,
    native_cpu: &mut Cpu,
    native_memory: &mut FlatMemory,
    reference_cpu: &mut Cpu,
    reference_memory: &mut FlatMemory,
    budget: u32,
) -> u32 {
    let count = engine.run_block(native_cpu, native_memory, budget).unwrap();
    assert!(count <= budget);
    for _ in 0..count {
        assert!(step(reference_cpu, reference_memory).unwrap().is_some());
    }
    assert_eq!(*native_cpu, *reference_cpu);
    for address in u16::MIN..=u16::MAX {
        assert_eq!(
            native_memory.peek(address),
            reference_memory.peek(address),
            "memory at {address:#06x}"
        );
    }
    count
}

#[test]
fn native_countdown_matches_every_block_exit() {
    let mut engine = NativeEngine::new().unwrap();
    let mut native_cpu = Cpu::default();
    let mut native_memory = FlatMemory::default();
    native_memory
        .load(0, &[0x3e, 3, 0x3d, 0x20, 0xfd, 0x32, 0, 0x80, 0x76])
        .unwrap();
    native_memory.load(0x8000, &[0xff]).unwrap();
    let mut reference_cpu = native_cpu.clone();
    let mut reference_memory = native_memory.clone();
    let mut remaining = 9;
    for (expected_count, expected_pc, expected_cycles) in
        [(3, 2, 23), (2, 2, 39), (2, 5, 50), (1, 8, 63), (1, 9, 67)]
    {
        let count = compare_block(
            &mut engine,
            &mut native_cpu,
            &mut native_memory,
            &mut reference_cpu,
            &mut reference_memory,
            remaining,
        );
        assert_eq!(count, expected_count);
        assert_eq!(
            (native_cpu.pc, native_cpu.t_states),
            (expected_pc, expected_cycles)
        );
        remaining -= count;
    }
    assert!(native_cpu.halted);
    assert_eq!(native_memory.peek(0x8000), 0);
    assert_eq!(remaining, 0);
    assert_eq!(
        engine
            .run_block(&mut native_cpu, &mut native_memory, 1)
            .unwrap(),
        0
    );
}

#[test]
fn store_ends_the_block_before_the_instruction_it_overwrites() {
    let mut engine = NativeEngine::new().unwrap();
    let mut native_cpu = Cpu::default();
    let mut native_memory = FlatMemory::default();
    native_memory.load(0, &[0x3e, 0x76, 0x32, 5, 0, 0]).unwrap();
    let mut reference_cpu = native_cpu.clone();
    let mut reference_memory = native_memory.clone();

    for (expected_count, expected_pc) in [(2, 5), (1, 6)] {
        assert_eq!(
            compare_block(
                &mut engine,
                &mut native_cpu,
                &mut native_memory,
                &mut reference_cpu,
                &mut reference_memory,
                8,
            ),
            expected_count
        );
        assert_eq!(native_cpu.pc, expected_pc);
        assert_eq!(native_memory.peek(5), 0x76);
    }
    assert!(native_cpu.halted);
    assert_eq!(native_cpu.t_states, 24);
}

#[test]
fn native_call_replays_wrapping_stack_writes() {
    let mut engine = NativeEngine::new().unwrap();
    let mut native_cpu = Cpu {
        pc: 0x1234,
        sp: 1,
        ..Cpu::default()
    };
    let mut native_memory = FlatMemory::default();
    native_memory.load(0x1234, &[0xcd, 0, 0x20]).unwrap();
    let mut reference_cpu = native_cpu.clone();
    let mut reference_memory = native_memory.clone();
    assert_eq!(
        compare_block(
            &mut engine,
            &mut native_cpu,
            &mut native_memory,
            &mut reference_cpu,
            &mut reference_memory,
            3,
        ),
        1
    );
    assert_eq!((native_cpu.pc, native_cpu.sp), (0x2000, 0xffff));
    assert_eq!(
        (native_memory.peek(0xffff), native_memory.peek(0)),
        (0x37, 0x12)
    );
}

#[test]
fn budget_limits_execution_and_unsupported_instructions_leave_pc_intact() {
    let mut engine = NativeEngine::new().unwrap();
    let mut cpu = Cpu::default();
    let mut memory = FlatMemory::default();
    memory.load(0, &[0, 0, 0, 0xc9, 0xed]).unwrap();
    assert_eq!(engine.run_block(&mut cpu, &mut memory, 0).unwrap(), 0);
    assert_eq!(cpu, Cpu::default());
    assert_eq!(engine.run_block(&mut cpu, &mut memory, 1).unwrap(), 1);
    assert_eq!((cpu.pc, cpu.t_states), (1, 4));
    assert_eq!(engine.run_block(&mut cpu, &mut memory, 8).unwrap(), 2);
    assert_eq!((cpu.pc, cpu.t_states), (3, 12));
    let before = cpu.clone();
    assert!(matches!(
        engine.run_block(&mut cpu, &mut memory, 8),
        Err(NativeError::UnsupportedInstruction {
            pc: 3,
            opcode: 0xc9
        })
    ));
    assert_eq!(cpu, before);
    cpu.pc = 4;
    let before = cpu.clone();
    assert!(matches!(
        engine.run_block(&mut cpu, &mut memory, 8),
        Err(NativeError::Decode(error)) if error.pc == 4 && error.opcode == 0xed
    ));
    assert_eq!(cpu, before);
}

#[test]
fn native_decrement_preserves_carry_and_handles_overflow_and_wrap() {
    let mut engine = NativeEngine::new().unwrap();
    for value in [0, 1, 0x10, 0x80] {
        let mut native_cpu = Cpu {
            r: 0xff,
            t_states: u64::MAX - 3,
            ..Cpu::default()
        };
        native_cpu.main.a = value;
        native_cpu.main.f = flags::C;
        let mut native_memory = FlatMemory::default();
        native_memory.load(0, &[0x3d]).unwrap();
        let mut reference_cpu = native_cpu.clone();
        let mut reference_memory = native_memory.clone();
        assert_eq!(
            compare_block(
                &mut engine,
                &mut native_cpu,
                &mut native_memory,
                &mut reference_cpu,
                &mut reference_memory,
                1,
            ),
            1
        );
        assert_eq!(native_cpu.r, 0x80);
        assert_eq!(native_cpu.t_states, 0);
    }
}
