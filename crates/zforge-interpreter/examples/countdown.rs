use zforge_core::{Cpu, FlatMemory, Memory};
use zforge_interpreter::step;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut memory = FlatMemory::default();
    memory.load(
        0,
        &[
            0x3e, 0x03, // LD A,3
            0x3d, // DEC A
            0x20, 0xfd, // JR NZ back to DEC A
            0x32, 0x00, 0x80, // LD (0x8000),A
            0x76, // HALT
        ],
    )?;
    let mut cpu = Cpu::default();
    while let Some(record) = step(&mut cpu, &mut memory)? {
        println!(
            "{:04x} {:?} ({} T-states)",
            record.pc, record.instruction, record.t_states
        );
    }
    println!(
        "RAM[0x8000] = {}, total T-states = {}",
        memory.read(0x8000),
        cpu.t_states
    );
    Ok(())
}
