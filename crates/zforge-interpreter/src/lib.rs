//! Instruction-at-a-time execution for the supported Z80 subset.

use zforge_core::{Cpu, Memory, flags};
use zforge_decoder::{Condition, DecodeError, Instruction, Operand8, Register16, decode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Step {
    pub pc: u16,
    pub instruction: Instruction,
    pub t_states: u8,
}

/// Execute one instruction and return a record for callers collecting traces.
/// A halted CPU returns `Ok(None)` until interrupt support is implemented.
/// On decode failure the opcode read has occurred, but CPU state is unchanged.
pub fn step(cpu: &mut Cpu, memory: &mut impl Memory) -> Result<Option<Step>, DecodeError> {
    if cpu.halted {
        return Ok(None);
    }

    let pc = cpu.pc;
    let decoded = decode(memory, pc)?;
    cpu.pc = decoded.next_pc;
    cpu.r = (cpu.r & 0x80) | (cpu.r.wrapping_add(1) & 0x7f);
    let t_states = execute(cpu, memory, decoded.instruction);
    cpu.t_states = cpu.t_states.wrapping_add(u64::from(t_states));
    Ok(Some(Step {
        pc,
        instruction: decoded.instruction,
        t_states,
    }))
}

fn execute(cpu: &mut Cpu, memory: &mut impl Memory, instruction: Instruction) -> u8 {
    use Instruction::*;

    match instruction {
        Nop => 4,
        Halt => {
            cpu.halted = true;
            4
        }
        Load8(dst, src) => {
            let value = read8(cpu, memory, src);
            write8(cpu, memory, dst, value);
            if dst == Operand8::AtHl || src == Operand8::AtHl {
                7
            } else {
                4
            }
        }
        LoadImmediate8(dst, value) => {
            write8(cpu, memory, dst, value);
            if dst == Operand8::AtHl { 10 } else { 7 }
        }
        LoadImmediate16(dst, value) => {
            *match dst {
                Register16::Bc => &mut cpu.main.bc,
                Register16::De => &mut cpu.main.de,
                Register16::Hl => &mut cpu.main.hl,
                Register16::Sp => &mut cpu.sp,
            } = value;
            10
        }
        LoadA(address) => {
            cpu.main.a = memory.read(address);
            13
        }
        StoreA(address) => {
            memory.write(address, cpu.main.a);
            13
        }
        Increment(operand) | Decrement(operand) => {
            let value = read8(cpu, memory, operand);
            let (result, arithmetic) = if matches!(instruction, Increment(_)) {
                let result = value.wrapping_add(1);
                (result, add_flags(value, 1, result))
            } else {
                let result = value.wrapping_sub(1);
                (result, sub_flags(value, 1, result))
            };
            cpu.main.f = (arithmetic & !flags::C) | (cpu.main.f & flags::C);
            write8(cpu, memory, operand, result);
            if operand == Operand8::AtHl { 11 } else { 4 }
        }
        AddImmediate(value) => {
            let a = cpu.main.a;
            cpu.main.a = a.wrapping_add(value);
            cpu.main.f = add_flags(a, value, cpu.main.a);
            7
        }
        SubtractImmediate(value) | CompareImmediate(value) => {
            let a = cpu.main.a;
            let result = a.wrapping_sub(value);
            cpu.main.f = sub_flags(a, value, result);
            if matches!(instruction, CompareImmediate(_)) {
                cpu.main.f =
                    (cpu.main.f & !(flags::X | flags::Y)) | (value & (flags::X | flags::Y));
            } else {
                cpu.main.a = result;
            }
            7
        }
        Jump(address) => {
            cpu.pc = address;
            10
        }
        JumpRelative(condition, offset) => {
            let f = cpu.main.f;
            let taken = match condition {
                Condition::Always => true,
                Condition::NotZero => f & flags::Z == 0,
                Condition::Zero => f & flags::Z != 0,
                Condition::NotCarry => f & flags::C == 0,
                Condition::Carry => f & flags::C != 0,
            };
            if taken {
                cpu.pc = cpu.pc.wrapping_add_signed(i16::from(offset));
                12
            } else {
                7
            }
        }
        Call(address) => {
            let [lo, hi] = cpu.pc.to_le_bytes();
            cpu.sp = cpu.sp.wrapping_sub(1);
            memory.write(cpu.sp, hi);
            cpu.sp = cpu.sp.wrapping_sub(1);
            memory.write(cpu.sp, lo);
            cpu.pc = address;
            17
        }
        Return => {
            let lo = memory.read(cpu.sp);
            cpu.sp = cpu.sp.wrapping_add(1);
            let hi = memory.read(cpu.sp);
            cpu.sp = cpu.sp.wrapping_add(1);
            cpu.pc = u16::from_le_bytes([lo, hi]);
            10
        }
    }
}

fn read8(cpu: &Cpu, memory: &mut impl Memory, operand: Operand8) -> u8 {
    match operand {
        Operand8::A => cpu.main.a,
        Operand8::B => (cpu.main.bc >> 8) as u8,
        Operand8::C => cpu.main.bc as u8,
        Operand8::D => (cpu.main.de >> 8) as u8,
        Operand8::E => cpu.main.de as u8,
        Operand8::H => (cpu.main.hl >> 8) as u8,
        Operand8::L => cpu.main.hl as u8,
        Operand8::AtHl => memory.read(cpu.main.hl),
    }
}

fn write8(cpu: &mut Cpu, memory: &mut impl Memory, operand: Operand8, value: u8) {
    let byte = u16::from(value);
    match operand {
        Operand8::A => cpu.main.a = value,
        Operand8::B => cpu.main.bc = (cpu.main.bc & 0xff) | (byte << 8),
        Operand8::C => cpu.main.bc = (cpu.main.bc & 0xff00) | byte,
        Operand8::D => cpu.main.de = (cpu.main.de & 0xff) | (byte << 8),
        Operand8::E => cpu.main.de = (cpu.main.de & 0xff00) | byte,
        Operand8::H => cpu.main.hl = (cpu.main.hl & 0xff) | (byte << 8),
        Operand8::L => cpu.main.hl = (cpu.main.hl & 0xff00) | byte,
        Operand8::AtHl => memory.write(cpu.main.hl, value),
    }
}

fn result_flags(result: u8) -> u8 {
    (result & (flags::S | flags::X | flags::Y)) | if result == 0 { flags::Z } else { 0 }
}

fn add_flags(lhs: u8, rhs: u8, result: u8) -> u8 {
    let mut f = result_flags(result);
    if (lhs & 0x0f) + (rhs & 0x0f) > 0x0f {
        f |= flags::H;
    }
    if (lhs ^ result) & (rhs ^ result) & 0x80 != 0 {
        f |= flags::PV;
    }
    if result < lhs {
        f |= flags::C;
    }
    f
}

fn sub_flags(lhs: u8, rhs: u8, result: u8) -> u8 {
    let mut f = result_flags(result) | flags::N;
    if (lhs & 0x0f) < (rhs & 0x0f) {
        f |= flags::H;
    }
    if (lhs ^ rhs) & (lhs ^ result) & 0x80 != 0 {
        f |= flags::PV;
    }
    if lhs < rhs {
        f |= flags::C;
    }
    f
}
