mod cpu;
mod csr;
mod memory;
mod exception;

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
        let inst = match cpu.fetch() {
            Ok(i) => i,
            Err(e) => {
                e.get_trap(&mut cpu);
                if e.is_fatal() {
                    break;
                }
                0
            }
        };

        // Add 4 to the program counter
        cpu.pc += 4;

        // Decode & Execute
        if let Err(e) = cpu.decode_execute(inst) {
            e.get_trap(&mut cpu);
            if e.is_fatal() {
                break;
            }
        }

        if cpu.pc == 0 {
            break;
        }
    }
    cpu.dump_registers();
    cpu.dump_csr();
    Ok(())
}
