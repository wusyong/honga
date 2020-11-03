/// Set memory size to 128MiB.
pub const MEMORY_SIZE: u64 = 128 * 1024 * 1024;

/// The CPU contains registers, a program coutner, and memory.
pub struct Cpu {
    /// 32 64-bit integer registers.
    regs: [u64; 32],
    /// Program counter point to the the memory address of the next instruction that would be executed.
    pub pc: u64,
    /// Memory to store executable instructions.
    pub memory: Vec<u8>,
}

impl Cpu {
    /// Create a new `Cpu` object.
    pub fn new(binary: Vec<u8>) -> Self {
        let mut regs = [0; 32];
        // Set the register x2 with the size of a memory when a CPU is instantiated.
        regs[2] = MEMORY_SIZE;

        Self {
            regs,
            pc: 0,
            memory: binary,
        }
    }

    /// Print values in all registers (x0-x31).
    pub fn dump_registers(&self) {
        let abi = [
            "zero", " ra ", " sp ", " gp ", " tp ", " t0 ", " t1 ", " t2 ", " s0 ", " s1 ", " a0 ",
            " a1 ", " a2 ", " a3 ", " a4 ", " a5 ", " a6 ", " a7 ", " s2 ", " s3 ", " s4 ", " s5 ",
            " s6 ", " s7 ", " s8 ", " s9 ", " s10", " s11", " t3 ", " t4 ", " t5 ", " t6 ",
        ];

        for i in 0..32 {
            println!("x{:02}({})={:>#18x}", i, abi[i], self.regs[i],)
        }
    }

    /// Fetch the instruction from memory.
    pub fn fetch(&self) -> u32 {
        let index = self.pc as usize;
        return (self.memory[index] as u32)
            | ((self.memory[index + 1] as u32) << 8)
            | ((self.memory[index + 2] as u32) << 16)
            | ((self.memory[index + 3] as u32) << 24);
    }

    /// Decode and execute an instruction.
    pub fn decode_execute(&mut self, inst: u32) {
        let opcode = inst & 0x0000007f;
        let rd = ((inst & 0x00000f80) >> 7) as usize;
        let rs1 = ((inst & 0x000f8000) >> 15) as usize;
        let rs2 = ((inst & 0x01f00000) >> 20) as usize;

        // Emulate that register x0 is hardwired with all bits equal to 0.
        self.regs[0] = 0;

        match opcode {
            // I-type
            0x13 => {
                // addi
                let imm = ((inst & 0xfff00000) as i32 as i64 >> 20) as u64;
                self.regs[rd] = self.regs[rs1].wrapping_add(imm);
            }
            // S-type
            0x33 => {
                // add
                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
            }
            _ => {
                dbg!(format!("Opcode {:#x} isn't implemented yet.", opcode));
            }
        }
    }
}
