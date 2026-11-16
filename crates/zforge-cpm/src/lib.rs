//! Minimal CP/M transient program runner for the native code-generation demo.

use std::fmt;
use std::io::{self, Write};

use zforge_core::{Cpu, FlatMemory};
use zforge_native::{NativeEngine, NativeError};

const WARM_BOOT: u16 = 0;
const BDOS_ENTRY: u16 = 5;
const COM_ENTRY: u16 = 0x0100;
const INITIAL_SP: u16 = 0xfffe;
// Reserve the initial zero return address and space for the first CALL.
const MAX_IMAGE_SIZE: usize = (INITIAL_SP - 2 - COM_ENTRY) as usize;

#[derive(Debug)]
pub enum Error {
    ImageTooLarge,
    InstructionBudgetExhausted,
    Halted,
    UnsupportedBdos(u8),
    BdosReturnToEntry,
    UnterminatedString(u16),
    Console(io::Error),
    Native(NativeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageTooLarge => write!(f, "COM image exceeds {MAX_IMAGE_SIZE} bytes"),
            Self::InstructionBudgetExhausted => f.write_str("CPU instruction budget exhausted"),
            Self::Halted => f.write_str("CPU halted before CP/M exit"),
            Self::UnsupportedBdos(function) => write!(f, "unsupported BDOS function {function}"),
            Self::BdosReturnToEntry => f.write_str("BDOS cannot return to its own entry point"),
            Self::UnterminatedString(address) => write!(
                f,
                "BDOS string at {address:#06x} has no dollar terminator in guest RAM"
            ),
            Self::Console(error) => write!(f, "CP/M console output failed: {error}"),
            Self::Native(error) => write!(f, "native guest execution failed: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Console(error) => Some(error),
            Self::Native(error) => Some(error),
            _ => None,
        }
    }
}

/// Load an in-memory COM image at 0100h and execute it using the native backend.
///
/// A transfer to 0000h or BDOS function 0 exits the program. BDOS function 9
/// writes the dollar-terminated bytes at DE to `console` and returns through
/// the guest stack. T-states in the returned CPU cover guest instructions.
pub fn run(image: &[u8], console: &mut impl Write, instruction_budget: u32) -> Result<Cpu, Error> {
    if image.len() > MAX_IMAGE_SIZE {
        return Err(Error::ImageTooLarge);
    }
    let mut memory = FlatMemory::default();
    memory
        .load(COM_ENTRY, image)
        .map_err(|_| Error::ImageTooLarge)?;
    let mut cpu = Cpu {
        pc: COM_ENTRY,
        sp: INITIAL_SP,
        ..Cpu::default()
    };
    let mut engine = NativeEngine::new().map_err(Error::Native)?;
    let mut remaining = instruction_budget;

    loop {
        match cpu.pc {
            WARM_BOOT => return Ok(cpu),
            BDOS_ENTRY => match cpu.main.bc as u8 {
                0 => return Ok(cpu),
                9 => {
                    let lo = memory.peek(cpu.sp);
                    let hi = memory.peek(cpu.sp.wrapping_add(1));
                    let return_pc = u16::from_le_bytes([lo, hi]);
                    if return_pc == BDOS_ENTRY {
                        return Err(Error::BdosReturnToEntry);
                    }
                    print_string(&memory, cpu.main.de, console)?;
                    cpu.sp = cpu.sp.wrapping_add(2);
                    cpu.pc = return_pc;
                    continue;
                }
                function => return Err(Error::UnsupportedBdos(function)),
            },
            _ => {}
        }
        if cpu.halted {
            return Err(Error::Halted);
        }
        if remaining == 0 {
            return Err(Error::InstructionBudgetExhausted);
        }
        let retired = engine
            .run_block(&mut cpu, &mut memory, remaining)
            .map_err(Error::Native)?;
        remaining -= retired;
    }
}

fn print_string(memory: &FlatMemory, start: u16, console: &mut impl Write) -> Result<(), Error> {
    let mut bytes = Vec::new();
    for offset in 0..=u16::MAX {
        let value = memory.peek(start.wrapping_add(offset));
        if value == b'$' {
            return console.write_all(&bytes).map_err(Error::Console);
        }
        bytes.push(value);
    }
    Err(Error::UnterminatedString(start))
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    #[test]
    fn returns_from_print_string_and_reaches_warm_boot_within_budget() {
        let image = [
            0x11, 0x0b, 0x01, // LD DE,010Bh
            0x0e, 0x09, // LD C,9
            0xcd, 0x05, 0x00, // CALL 0005h
            0xc3, 0x00, 0x00, // JP 0000h
            b'H', b'i', 0xff, b'$', b'!',
        ];
        let mut console = Vec::new();
        let cpu = run(&image, &mut console, 4).unwrap();
        assert_eq!(console, [b'H', b'i', 0xff]);
        assert_eq!(cpu.pc, WARM_BOOT);
        assert_eq!(cpu.sp, INITIAL_SP);
        assert_eq!(cpu.t_states, 44);
        assert_eq!(cpu.r, 4);
    }

    #[test]
    fn bdos_function_zero_exits() {
        let image = [0x0e, 0x00, 0xcd, 0x05, 0x00]; // LD C,0; CALL 0005h
        let cpu = run(&image, &mut io::sink(), 2).unwrap();
        assert_eq!(cpu.pc, BDOS_ENTRY);
        assert_eq!(cpu.t_states, 24);
    }

    #[test]
    fn unknown_bdos_function_reports_an_error() {
        let image = [0x0e, 0x0a, 0xcd, 0x05, 0x00]; // LD C,10; CALL 0005h
        assert!(matches!(
            run(&image, &mut io::sink(), 2),
            Err(Error::UnsupportedBdos(10))
        ));
    }

    #[test]
    fn reports_missing_string_terminator_without_partial_output() {
        let image = [
            0x11, 0x08, 0x01, // LD DE,0108h
            0x0e, 0x09, // LD C,9
            0xcd, 0x05, 0x00, // CALL 0005h
            b'H', b'i',
        ];
        let mut console = Vec::new();
        assert!(matches!(
            run(&image, &mut console, 3),
            Err(Error::UnterminatedString(0x0108))
        ));
        assert!(console.is_empty());
    }

    #[test]
    fn instruction_budget_stops_a_guest_loop() {
        let image = [0xc3, 0x00, 0x01]; // JP 0100h
        assert!(matches!(
            run(&image, &mut io::sink(), 3),
            Err(Error::InstructionBudgetExhausted)
        ));
    }

    #[test]
    fn a_bdos_return_requires_progress_through_guest_code() {
        let image = [
            0x3e, 0x05, // LD A,5
            0x32, 0xfe, 0xff, // LD (0FFFEh),A
            0x11, 0x0d, 0x01, // LD DE,010Dh
            0x0e, 0x09, // LD C,9
            0xc3, 0x05, 0x00, // JP 0005h
            b'H', b'i', b'$',
        ];
        let mut console = Vec::new();
        assert!(matches!(
            run(&image, &mut console, 5),
            Err(Error::BdosReturnToEntry)
        ));
        assert!(console.is_empty());
    }
}
