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
        let funct3 = (inst & 0x00007000) >> 12;
        let funct7 = (inst & 0xfe000000) >> 25;

        // Emulate that register x0 is hardwired with all bits equal to 0.
        self.regs[0] = 0;

        match opcode {
            // I-type
            0x13 => {
                // ADDI
                let imm = ((inst & 0xfff00000) as i32 as i64 >> 20) as u64;
                let shamt = (imm & 0x3f) as u32;
                match funct3 {
                    // ADDI
                    0x0 => self.regs[rd] = self.regs[rs1].wrapping_add(imm),
                    // SLLI
                    0x1 => self.regs[rd] = self.regs[rs1] << shamt,
                    // SLTI
                    0x2 => self.regs[rd] = ((self.regs[rs1] as i64) < (imm as i64)) as u64,
                    // SLTIU
                    0x3 => self.regs[rd] = (self.regs[rs1] < imm) as u64,
                    // XORI
                    0x4 => self.regs[rd] = self.regs[rs1] ^ imm,
                    0x5 => match funct7 {
                        // SRLI
                        0x00 => self.regs[rd] = self.regs[rs1].wrapping_shr(shamt),
                        // SRAI
                        0x20 => self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64,
                        _ => unreachable!(),
                    },
                    // ORI
                    0x6 => self.regs[rd] = self.regs[rs1] | imm,
                    // ANDI
                    0x7 => self.regs[rd] = self.regs[rs1] & imm,
                    _ => todo!(),
                }
            }
            // S-type
            0x33 => {
                let shamt = ((self.regs[rs2] & 0x3f) as u64) as u32;
                match (funct3, funct7) {
                    // ADD
                    (0x0, 0x00) => self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]),
                    // SUB
                    (0x0, 0x20) => self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]),
                    // SLL
                    (0x1, 0x00) => self.regs[rd] = self.regs[rs1].wrapping_shl(shamt),
                    // SLT
                    (0x2, 0x00) => self.regs[rd] = ((self.regs[rs1] as i64) < (self.regs[rs2] as i64)) as u64,
                    // SLTU
                    (0x3, 0x00) => self.regs[rd] = (self.regs[rs1] < self.regs[rs2]) as u64,
                    // XOR
                    (0x4, 0x00) => self.regs[rd] = self.regs[rs1] ^ self.regs[rs2],
                    // SRL
                    (0x5, 0x00) => self.regs[rd] = self.regs[rs1].wrapping_shr(shamt),
                    // SRA
                    (0x5, 0x20) => self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64,
                    // OR
                    (0x6, 0x00) => self.regs[rd] = self.regs[rs1] | self.regs[rs2],
                    // AND
                    (0x7, 0x00) => self.regs[rd] = self.regs[rs1] & self.regs[rs2],
                    _ => todo!(),
                }
            }
            _ => {
                dbg!(format!("Opcode {:#x} isn't implemented yet.", opcode));
            }
        }
    }
}
