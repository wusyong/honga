mod cpu;
mod memory;

use crate::cpu::Cpu;

use std::io::prelude::*;

fn main() -> std::io::Result<()> {
    // Read binary to memory.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        panic!("Usage: cargo run <filename>");
    }
    let mut file = std::fs::File::open(&args[1])?;
    let mut binary = Vec::new();
    file.read_to_end(&mut binary)?;

    let mut cpu = Cpu::new(binary);
    // Instruction cycle
    loop {
        // Fetch instruction
        let inst = cpu.fetch();

        // Add 4 to the program counter
        cpu.pc += 4;

        // Decode & Execute
        cpu.decode_execute(inst);

        if cpu.pc == 0 {
            break;
        }
    }
    cpu.dump_registers();

    Ok(())
}
