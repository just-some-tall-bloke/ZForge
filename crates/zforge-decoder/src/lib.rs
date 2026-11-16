//! Typed decoding of the initial unprefixed Z80 instruction subset.

use zforge_core::Memory;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operand8 {
    B,
    C,
    D,
    E,
    H,
    L,
    AtHl,
    A,
}

impl Operand8 {
    fn from_field(opcode: u8) -> Self {
        [
            Self::B,
            Self::C,
            Self::D,
            Self::E,
            Self::H,
            Self::L,
            Self::AtHl,
            Self::A,
        ][usize::from(opcode & 7)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Register16 {
    Bc,
    De,
    Hl,
    Sp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Condition {
    Always,
    NotZero,
    Zero,
    NotCarry,
    Carry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Instruction {
    Nop,
    Halt,
    Load8(Operand8, Operand8),
    LoadImmediate8(Operand8, u8),
    LoadImmediate16(Register16, u16),
    LoadA(u16),
    StoreA(u16),
    Increment(Operand8),
    Decrement(Operand8),
    AddImmediate(u8),
    SubtractImmediate(u8),
    CompareImmediate(u8),
    Jump(u16),
    JumpRelative(Condition, i8),
    Call(u16),
    Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decoded {
    pub instruction: Instruction,
    pub next_pc: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeError {
    pub pc: u16,
    pub opcode: u8,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unsupported opcode {:#04x} at {:#06x}",
            self.opcode, self.pc
        )
    }
}

impl std::error::Error for DecodeError {}

/// Fetch one instruction, wrapping at the end of the Z80 address space.
pub fn decode(memory: &mut impl Memory, pc: u16) -> Result<Decoded, DecodeError> {
    decode_with(pc, |address| memory.read(address))
}

/// Decode using a caller-supplied byte source, such as a read-only view of RAM.
/// Requests bytes in instruction order and wraps addresses at 16 bits.
pub fn decode_with(pc: u16, mut read: impl FnMut(u16) -> u8) -> Result<Decoded, DecodeError> {
    use Instruction::*;

    let mut next_pc = pc;
    let mut byte = || {
        let value = read(next_pc);
        next_pc = next_pc.wrapping_add(1);
        value
    };
    let opcode = byte();
    let operand = Operand8::from_field(opcode >> 3);
    let mut word = || u16::from_le_bytes([byte(), byte()]);
    let instruction = match opcode {
        0x00 => Nop,
        0x76 => Halt,
        0x40..=0x7f => Load8(operand, Operand8::from_field(opcode)),
        0x06 | 0x0e | 0x16 | 0x1e | 0x26 | 0x2e | 0x36 | 0x3e => LoadImmediate8(operand, byte()),
        0x01 => LoadImmediate16(Register16::Bc, word()),
        0x11 => LoadImmediate16(Register16::De, word()),
        0x21 => LoadImmediate16(Register16::Hl, word()),
        0x31 => LoadImmediate16(Register16::Sp, word()),
        0x3a => LoadA(word()),
        0x32 => StoreA(word()),
        0x04 | 0x0c | 0x14 | 0x1c | 0x24 | 0x2c | 0x34 | 0x3c => Increment(operand),
        0x05 | 0x0d | 0x15 | 0x1d | 0x25 | 0x2d | 0x35 | 0x3d => Decrement(operand),
        0xc6 => AddImmediate(byte()),
        0xd6 => SubtractImmediate(byte()),
        0xfe => CompareImmediate(byte()),
        0xc3 => Jump(word()),
        0x18 => JumpRelative(Condition::Always, byte() as i8),
        0x20 => JumpRelative(Condition::NotZero, byte() as i8),
        0x28 => JumpRelative(Condition::Zero, byte() as i8),
        0x30 => JumpRelative(Condition::NotCarry, byte() as i8),
        0x38 => JumpRelative(Condition::Carry, byte() as i8),
        0xcd => Call(word()),
        0xc9 => Return,
        _ => return Err(DecodeError { pc, opcode }),
    };
    Ok(Decoded {
        instruction,
        next_pc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zforge_core::FlatMemory;

    #[test]
    fn fetches_little_endian_operands_across_address_wrap() {
        let mut memory = FlatMemory::default();
        memory.load(0xfffe, &[0x21, 0x34]).unwrap();
        memory.write(0, 0x12);
        assert_eq!(
            decode(&mut memory, 0xfffe),
            Ok(Decoded {
                instruction: Instruction::LoadImmediate16(Register16::Hl, 0x1234),
                next_pc: 1,
            })
        );
    }

    #[test]
    fn reports_unsupported_prefix_at_its_start_address() {
        let mut memory = FlatMemory::default();
        memory.write(0x100, 0xdd);
        assert_eq!(
            decode(&mut memory, 0x100),
            Err(DecodeError {
                pc: 0x100,
                opcode: 0xdd
            })
        );
    }

    #[test]
    fn read_only_decode_fetches_just_the_instruction_bytes() {
        let mut memory = FlatMemory::default();
        memory.load(0xffff, &[0x18]).unwrap();
        memory.write(0, 0xfe);
        let mut addresses = Vec::new();
        let decoded = decode_with(0xffff, |address| {
            addresses.push(address);
            memory.peek(address)
        })
        .unwrap();

        assert_eq!(addresses, [0xffff, 0]);
        assert_eq!(
            decoded,
            Decoded {
                instruction: Instruction::JumpRelative(Condition::Always, -2),
                next_pc: 1,
            }
        );
    }
}
