use crate::memory::{Memory, MEMORY_BASE, MEMORY_SIZE};

/// The CPU contains registers, a program coutner, and memory.
pub struct Cpu {
    /// 32 64-bit integer registers.
    regs: [u64; 32],
    /// Program counter point to the the memory address of the next instruction that would be executed.
    pub pc: u64,
    /// Memory to store executable instructions.
    pub memory: Memory,
}

impl Cpu {
    /// Create a new `Cpu` object.
    pub fn new(binary: Vec<u8>) -> Self {
        let mut regs = [0; 32];
        // Set the register x2 with the size of a memory when a CPU is instantiated.
        regs[2] = MEMORY_SIZE + MEMORY_BASE;

        Self {
            regs,
            pc: MEMORY_BASE,
            memory: Memory::new(binary),
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
        self.memory.load(self.pc, 32) as u32
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
            0x03 => {
                let imm = ((inst as i32 as i64) >> 20) as u64;
                let address = self.regs[rs1].wrapping_add(imm);
                match funct3 {
                    // LB
                    0x0 => {
                        let value = self.memory.load(address, 8);
                        self.regs[rd] = value as i8 as i64 as u64;
                    }
                    // LH
                    0x1 => {
                        let value = self.memory.load(address, 16);
                        self.regs[rd] = value as i16 as i64 as u64;
                    }
                    // LW
                    0x2 => {
                        let value = self.memory.load(address, 32);
                        self.regs[rd] = value as i32 as i64 as u64;
                    }
                    // LD
                    0x3 => {
                        let value = self.memory.load(address, 64);
                        self.regs[rd] = value as i64 as u64;
                    }
                    // LBU
                    0x4 => {
                        let value = self.memory.load(address, 8);
                        self.regs[rd] = value;
                    }
                    // LHU
                    0x5 => {
                        let value = self.memory.load(address, 16);
                        self.regs[rd] = value;
                    }
                    // LWU
                    0x6 => {
                        let value = self.memory.load(address, 32);
                        self.regs[rd] = value;
                    }
                    _ => unreachable!(),
                }
            }
            0x13 => {
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
            // AUIPC
            0x17 => {
                let imm = (inst & 0xfffff000) as i32 as i64 as u64;
                self.regs[rd] = self.pc.wrapping_sub(4).wrapping_add(imm);
            }
            0x1b => {
                let imm = ((inst as i32 as i64) >> 20) as u64;
                let shamnt = (imm & 0x1f) as u32;
                match funct3 {
                    // ADDIW
                    0x0 => self.regs[rd] = self.regs[rs1].wrapping_add(imm) as i32 as i64 as u64,
                    // SLLIW
                    0x1 => self.regs[rd] = self.regs[rs1].wrapping_shl(shamnt) as i32 as i64 as u64,
                    0x5 => {
                        match funct7 {
                            // SRLIW
                            0x00 => {
                                self.regs[rd] = (self.regs[rs1] as u32).wrapping_shr(shamnt) as i32
                                    as i64 as u64
                            }
                            // SRAIW
                            0x20 => {
                                self.regs[rd] =
                                    (self.regs[rs1] as i32).wrapping_shr(shamnt) as i64 as u64
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => todo!(),
                }
            }
            0x23 => {
                let imm = (((inst & 0xfe000000) as i32 as i64 >> 20) as u64)
                    | ((inst >> 7) & 0x1f) as u64;
                let address = self.regs[rs1].wrapping_add(imm);
                match funct3 {
                    // SB
                    0x0 => self.memory.store(address, 8, self.regs[rs2]),
                    // SH
                    0x1 => self.memory.store(address, 16, self.regs[rs2]),
                    // SW
                    0x2 => self.memory.store(address, 32, self.regs[rs2]),
                    // SD
                    0x3 => self.memory.store(address, 64, self.regs[rs2]),
                    _ => unreachable!(),
                }
            }
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
                    (0x2, 0x00) => {
                        self.regs[rd] = ((self.regs[rs1] as i64) < (self.regs[rs2] as i64)) as u64
                    }
                    // SLTU
                    (0x3, 0x00) => self.regs[rd] = (self.regs[rs1] < self.regs[rs2]) as u64,
                    // XOR
                    (0x4, 0x00) => self.regs[rd] = self.regs[rs1] ^ self.regs[rs2],
                    // SRL
                    (0x5, 0x00) => self.regs[rd] = self.regs[rs1].wrapping_shr(shamt),
                    // SRA
                    (0x5, 0x20) => {
                        self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64
                    }
                    // OR
                    (0x6, 0x00) => self.regs[rd] = self.regs[rs1] | self.regs[rs2],
                    // AND
                    (0x7, 0x00) => self.regs[rd] = self.regs[rs1] & self.regs[rs2],
                    _ => todo!(),
                }
            }
            // LUI
            0x37 => self.regs[rd] = (inst & 0xfffff000) as i32 as i64 as u64,
            0x3b => {
                let shamt = (self.regs[rs2] & 0x1f) as u32;
                match (funct3, funct7) {
                    // ADDW
                    (0x0, 0x00) => {
                        self.regs[rd] =
                            self.regs[rs1].wrapping_add(self.regs[rs2]) as i32 as i64 as u64
                    }
                    // SUBW
                    (0x0, 0x20) => {
                        self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]) as i32 as u64
                    }
                    // SLLW
                    (0x1, 0x00) => {
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shl(shamt) as i32 as u64
                    }
                    // SRLW
                    (0x5, 0x00) => {
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shr(shamt) as i32 as u64
                    }
                    // SRAW
                    (0x5, 0x20) => {
                        self.regs[rd] = ((self.regs[rs1] as i32) >> (shamt as i32)) as u64
                    }
                    _ => todo!(),
                }
            }
            0x63 => {
                let imm = ((inst & 0x80000000) as i32 as i64 >> 19) as u64
                    | ((inst & 20) & 0x7e0) as u64
                    | ((inst & 0x80) << 4) as u64
                    | ((inst >> 7) & 0x1e) as u64;
                match funct3 {
                    // BEQ
                    0x0 => {
                        if self.regs[rs1] == self.regs[rs2] {
                            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
                        }
                    }
                    // BNQ
                    0x1 => {
                        if self.regs[rs1] != self.regs[rs2] {
                            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
                        }
                    }
                    // BLT
                    0x4 => {
                        if (self.regs[rs1] as i64) < (self.regs[rs2] as i64) {
                            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
                        }
                    }
                    // BGE
                    0x5 => {
                        if (self.regs[rs1] as i64) >= (self.regs[rs2] as i64) {
                            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
                        }
                    }
                    // BLTU
                    0x6 => {
                        if self.regs[rs1] < self.regs[rs2] {
                            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
                        }
                    }
                    // BGEU
                    0x7 => {
                        if self.regs[rs1] >= self.regs[rs2] {
                            self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
                        }
                    }
                    _ => todo!(),
                }
            }
            // JALR
            0x67 => {
                self.regs[rd] = self.pc;

                let imm = ((((inst & 0xfff00000) as i32) as i64) >> 20) as u64;
                self.pc = self.regs[rs1].wrapping_add(imm) & !1;
            }
            // JAL
            0x6f => {
                self.regs[rd] = self.pc;

                let imm = (((inst & 0x80000000) as i32 as i64 >> 11) as u64)
                    | ((inst >> 20) & 0x7fe) as u64
                    | ((inst >> 9) & 0x800) as u64
                    | (inst & 0xff000) as u64;
                self.pc = self.pc.wrapping_sub(4).wrapping_add(imm);
            }
            _ => {
                dbg!(format!("Opcode {:#x} isn't implemented yet.", opcode));
            }
        }
    }
}
