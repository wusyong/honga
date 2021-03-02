mod bus;
mod cpu;
mod csr;
mod exception;
mod interrupt;

use crate::cpu::Cpu;

use std::io::prelude::*;

fn main() -> std::io::Result<()> {
    // Read binary to memory.
    let args: Vec<String> = std::env::args().collect();
    if (args.len() != 2) && (args.len() != 3) {
        panic!("Usage: cargo run <filename> <(option) image>");
    }
    let mut file = std::fs::File::open(&args[1])?;
    let mut binary = Vec::new();
    file.read_to_end(&mut binary)?;

    let mut image = Vec::new();
    if args.len() == 3 {
        let mut file = std::fs::File::open(&args[2])?;
        file.read_to_end(&mut image)?;
    }

    let mut cpu = Cpu::new(binary, image);
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

        match cpu.check_pending_interrupt() {
            Some(interrupt) => interrupt.get_trap(&mut cpu),
            None => {}
        }
    }
    cpu.dump_registers();
    cpu.dump_csr();
    Ok(())
}
