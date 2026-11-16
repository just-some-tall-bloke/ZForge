use zforge_core::{Cpu, FlatMemory};
use zforge_native::NativeEngine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = NativeEngine::new()?;
    let mut cpu = Cpu::default();
    let mut memory = FlatMemory::default();
    memory.load(0, &[0x3e, 3, 0x3d, 0x20, 0xfd, 0x32, 0, 0x80, 0x76])?;
    let mut remaining = 9;
    while !cpu.halted && remaining != 0 {
        remaining -= engine.run_block(&mut cpu, &mut memory, remaining)?;
    }
    if !cpu.halted {
        return Err("countdown exhausted its instruction budget".into());
    }
    println!(
        "RAM[0x8000] = {}, total T-states = {}",
        memory.peek(0x8000),
        cpu.t_states
    );
    Ok(())
}
