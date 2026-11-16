//! Z80 architectural state and the memory interface used by the CPU.

pub const ADDRESS_SPACE_SIZE: usize = 1 << 16;

pub mod flags {
    pub const C: u8 = 0x01;
    pub const N: u8 = 0x02;
    pub const PV: u8 = 0x04;
    pub const X: u8 = 0x08;
    pub const H: u8 = 0x10;
    pub const Y: u8 = 0x20;
    pub const Z: u8 = 0x40;
    pub const S: u8 = 0x80;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Registers {
    pub a: u8,
    pub f: u8,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
}

impl Registers {
    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }

    pub fn set_af(&mut self, value: u16) {
        [self.a, self.f] = value.to_be_bytes();
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InterruptMode {
    #[default]
    Mode0,
    Mode1,
    Mode2,
}

/// The default is a deterministic starting state for tests and programs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cpu {
    pub main: Registers,
    pub alternate: Registers,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub interrupt_mode: InterruptMode,
    pub halted: bool,
    pub ei_delay: bool,
    pub t_states: u64,
}

/// A 16-bit address space. Reads take a mutable reference to allow later
/// adapters to observe accesses and implement read side effects.
pub trait Memory {
    fn read(&mut self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
}

#[derive(Clone, Debug)]
pub struct FlatMemory {
    bytes: Box<[u8]>,
}

impl Default for FlatMemory {
    fn default() -> Self {
        Self {
            bytes: vec![0; ADDRESS_SPACE_SIZE].into_boxed_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoadError {
    pub address: u16,
    pub length: usize,
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} bytes at {:#06x} exceed the address space",
            self.length, self.address
        )
    }
}

impl std::error::Error for LoadError {}

impl FlatMemory {
    /// Inspect a RAM byte without performing an emulated memory access.
    pub fn peek(&self, address: u16) -> u8 {
        self.bytes[usize::from(address)]
    }

    /// Load a contiguous image. An image that crosses the end of RAM is
    /// rejected before any bytes are written.
    pub fn load(&mut self, address: u16, image: &[u8]) -> Result<(), LoadError> {
        let start = usize::from(address);
        if image.len() > ADDRESS_SPACE_SIZE - start {
            return Err(LoadError {
                address,
                length: image.len(),
            });
        }
        self.bytes[start..start + image.len()].copy_from_slice(image);
        Ok(())
    }
}

impl Memory for FlatMemory {
    fn read(&mut self, address: u16) -> u8 {
        self.peek(address)
    }

    fn write(&mut self, address: u16, value: u8) {
        self.bytes[usize::from(address)] = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn af_round_trips() {
        let mut registers = Registers::default();
        registers.set_af(0x12f8);
        assert_eq!((registers.a, registers.f), (0x12, 0xf8));
        assert_eq!(registers.af(), 0x12f8);
    }

    #[test]
    fn image_load_checks_bounds_before_writing() {
        let mut memory = FlatMemory::default();
        memory.load(0xffff, &[0x42]).unwrap();
        assert!(memory.load(0xffff, &[1, 2]).is_err());
        assert_eq!(memory.read(0xffff), 0x42);
        assert_eq!(memory.read(0), 0);
    }
}
