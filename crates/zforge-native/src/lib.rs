//! A bounded x86-64 proof of concept for translating a subset of Z80 to native code.

use std::{error::Error, fmt};

pub use zforge_decoder::DecodeError;

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::NativeEngine;

#[derive(Debug)]
pub enum NativeError {
    UnsupportedHost,
    Decode(DecodeError),
    UnsupportedInstruction { pc: u16, opcode: u8 },
    Codegen(String),
}

impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => write!(f, "native translation requires an x86-64 host"),
            Self::Decode(error) => error.fmt(f),
            Self::UnsupportedInstruction { pc, opcode } => {
                write!(
                    f,
                    "native translation does not support opcode {opcode:#04x} at {pc:#06x}"
                )
            }
            Self::Codegen(error) => write!(f, "native code generation failed: {error}"),
        }
    }
}

impl Error for NativeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub struct NativeEngine {
    _unsupported: std::convert::Infallible,
}

#[cfg(not(target_arch = "x86_64"))]
impl NativeEngine {
    pub fn new() -> Result<Self, NativeError> {
        Err(NativeError::UnsupportedHost)
    }

    pub fn run_block(
        &mut self,
        _cpu: &mut zforge_core::Cpu,
        _memory: &mut zforge_core::FlatMemory,
        _budget: u32,
    ) -> Result<u32, NativeError> {
        Err(NativeError::UnsupportedHost)
    }
}
