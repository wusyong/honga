use crate::bus::{Bus, MEMORY_BASE, MEMORY_SIZE, PLIC_SCLAIM, UART_IRQ};
use crate::csr::*;
use crate::exception::Exception;
use crate::interrupt::Interrupt;

// MIP fields.
const MIP_SSIP: u64 = 1 << 1;
const MIP_MSIP: u64 = 1 << 3;
const MIP_STIP: u64 = 1 << 5;
const MIP_MTIP: u64 = 1 << 7;
const MIP_SEIP: u64 = 1 << 9;
const MIP_MEIP: u64 = 1 << 11;

/// Privileged mode.
#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum Mode {
    User = 0,
    Supervisor = 1,
    Machine = 3,
}

/// The CPU contains registers, a program coutner, and memory.
pub struct Cpu {
    /// 32 64-bit integer registers.
    regs: [u64; 32],
    /// Program counter point to the the memory address of the next instruction that would be executed.
    pub pc: u64,
    /// Memory to store executable instructions.
    pub bus: Bus,
    /// Control & status registers. RISC-V has 12-bit encoding space csr[11:0] which contain 4096
    /// csr.
    pub csr: [u64; 4096],
    /// Current privilege mode.
    pub mode: Mode,
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
            bus: Bus::new(binary),
            csr: [0; 4096],
            mode: Mode::Machine,
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

    /// Print values of selected CSRs.
    pub fn dump_csr(&self) {
        println!("sstatus={:>#18x}", self.load_csr(SSTATUS));
        println!("stvec ={:>#18x}", self.load_csr(STVEC));
        println!("sepc   ={:>#18x}", self.load_csr(SEPC));
        println!("scause ={:>#18x}", self.load_csr(SCAUSE));
        println!("mstatus={:>#18x}", self.load_csr(MSTATUS));
        println!("mtvec ={:>#18x}", self.load_csr(MTVEC));
        println!("mepc   ={:>#18x}", self.load_csr(MEPC));
        println!("mcause ={:>#18x}", self.load_csr(MCAUSE));
    }

    /// Load the value from the CSR
    pub fn load_csr(&self, address: usize) -> u64 {
        match address {
            SIE => self.csr[MIE] & self.csr[MIDELEG],
            _ => self.csr[address],
        }
    }

    /// Store the value to the CSR
    pub fn store_csr(&mut self, address: usize, value: u64) {
        match address {
            SIE => {
                self.csr[MIE] = (self.csr[MIE] & !self.csr[MIDELEG]) | (value & self.csr[MIDELEG])
            }
            _ => self.csr[address] = value,
        }
    }

    pub fn check_pending_interrupt(&mut self) -> Option<Interrupt> {
        match self.mode {
            Mode::Machine => {
                // Check if the MIE bit is enabled.
                if (self.load_csr(MSTATUS) >> 3) & 1 == 0 {
                    return None;
                }
            }
            Mode::Supervisor => {
                // Check if the SIE bit is enabled.
                if (self.load_csr(SSTATUS) >> 1) & 1 == 0 {
                    return None;
                }
            }
            _ => {}
        }

        // Check external interrupt for uart.
        let irq;
        if self.bus.uart.is_interrupting() {
            irq = UART_IRQ;
        } else {
            irq = 0;
        }

        if irq != 0 {
            self.bus
                .store(PLIC_SCLAIM, 32, irq)
                .expect("failed to write an IRQ to the PLIC_SCLAIM");
            self.store_csr(MIP, self.load_csr(MIP) | MIP_SEIP);
        }

        let pending = self.load_csr(MIE) & self.load_csr(MIP);

        if (pending & MIP_MEIP) != 0 {
            self.store_csr(MIP, self.load_csr(MIP) & !MIP_MEIP);
            return Some(Interrupt::MachineExternalInterrupt);
        }
        if (pending & MIP_MSIP) != 0 {
            self.store_csr(MIP, self.load_csr(MIP) & !MIP_MSIP);
            return Some(Interrupt::MachineSoftwareInterrupt);
        }
        if (pending & MIP_MTIP) != 0 {
            self.store_csr(MIP, self.load_csr(MIP) & !MIP_MTIP);
            return Some(Interrupt::MachineTimerInterrupt);
        }
        if (pending & MIP_SEIP) != 0 {
            self.store_csr(MIP, self.load_csr(MIP) & !MIP_SEIP);
            return Some(Interrupt::SupervisorExternalInterrupt);
        }
        if (pending & MIP_SSIP) != 0 {
            self.store_csr(MIP, self.load_csr(MIP) & !MIP_SSIP);
            return Some(Interrupt::SupervisorSoftwareInterrupt);
        }
        if (pending & MIP_STIP) != 0 {
            self.store_csr(MIP, self.load_csr(MIP) & !MIP_STIP);
            return Some(Interrupt::SupervisorTimerInterrupt);
        }
        None
    }

    /// Fetch the instruction from memory.
    pub fn fetch(&self) -> Result<u32, Exception> {
        match self.bus.load(self.pc, 32) {
            Ok(v) => Ok(v as u32),
            Err(_) => Err(Exception::InstructionAccessFault),
        }
    }

    /// Decode and execute an instruction.
    pub fn decode_execute(&mut self, inst: u32) -> Result<(), Exception> {
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
                        let value = self.bus.load(address, 8)?;
                        self.regs[rd] = value as i8 as i64 as u64;
                    }
                    // LH
                    0x1 => {
                        let value = self.bus.load(address, 16)?;
                        self.regs[rd] = value as i16 as i64 as u64;
                    }
                    // LW
                    0x2 => {
                        let value = self.bus.load(address, 32)?;
                        self.regs[rd] = value as i32 as i64 as u64;
                    }
                    // LD
                    0x3 => {
                        let value = self.bus.load(address, 64)?;
                        self.regs[rd] = value as i64 as u64;
                    }
                    // LBU
                    0x4 => {
                        let value = self.bus.load(address, 8)?;
                        self.regs[rd] = value;
                    }
                    // LHU
                    0x5 => {
                        let value = self.bus.load(address, 16)?;
                        self.regs[rd] = value;
                    }
                    // LWU
                    0x6 => {
                        let value = self.bus.load(address, 32)?;
                        self.regs[rd] = value;
                    }
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
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
                        _ => (),
                    },
                    // ORI
                    0x6 => self.regs[rd] = self.regs[rs1] | imm,
                    // ANDI
                    0x7 => self.regs[rd] = self.regs[rs1] & imm,
                    _ => (),
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
                            _ => {
                                println!(
                                    "Unsupported instruction: opcode {:x} funct3 {:x} funct7 {:x}",
                                    opcode, funct3, funct7
                                );
                                return Err(Exception::IllegalInstruction);
                            }
                        }
                    }
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x23 => {
                let imm = (((inst & 0xfe000000) as i32 as i64 >> 20) as u64)
                    | ((inst >> 7) & 0x1f) as u64;
                let address = self.regs[rs1].wrapping_add(imm);
                match funct3 {
                    // SB
                    0x0 => self.bus.store(address, 8, self.regs[rs2])?,
                    // SH
                    0x1 => self.bus.store(address, 16, self.regs[rs2])?,
                    // SW
                    0x2 => self.bus.store(address, 32, self.regs[rs2])?,
                    // SD
                    0x3 => self.bus.store(address, 64, self.regs[rs2])?,
                    _ => (),
                }
            }
            // RV64A: "A" standard extension for atomic instructions
            0x2f => {
                let funct5 = (funct7 & 0x7c) >> 2;
                let aq = (funct7 & 0x02) >> 1;
                let rl = funct7 & 0x01;

                match (funct3, funct5) {
                    // AMOADD.W
                    (0x2, 0x00) => {
                        self.regs[rd] = self.bus.load(self.regs[rs1], 32)?;
                        self.bus.store(
                            self.regs[rs1],
                            32,
                            self.regs[rd].wrapping_add(self.regs[rs2]),
                        )?;
                    }
                    // AMOADD.D
                    (0x3, 0x00) => {
                        self.regs[rd] = self.bus.load(self.regs[rs1], 64)?;
                        self.bus.store(
                            self.regs[rs1],
                            64,
                            self.regs[rd].wrapping_add(self.regs[rs2]),
                        )?;
                    }
                    // AMOSWAP.W
                    (0x2, 0x01) => {
                        self.regs[rd] = self.bus.load(self.regs[rs1], 32)?;
                        self.bus.store(self.regs[rs1], 32, self.regs[rs2])?;
                    }
                    // AMOSWAP.D
                    (0x3, 0x01) => {
                        self.regs[rd] = self.bus.load(self.regs[rs1], 64)?;
                        self.bus.store(self.regs[rs1], 64, self.regs[rs2])?;
                    }
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x} funct7 {:x}",
                            opcode, funct3, funct7
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x33 => {
                let shamt = ((self.regs[rs2] & 0x3f) as u64) as u32;
                match (funct3, funct7) {
                    // ADD
                    (0x0, 0x00) => self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]),
                    // MUL
                    (0x0, 0x01) => self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]),
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
                    // DIVU
                    (0x5, 0x01) => {
                        self.regs[rd] = match self.regs[rs2] {
                            0 => 0xffffffff_ffffffff,
                            _ => self.regs[rs1].wrapping_div(self.regs[rs2]),
                        }
                    }
                    // SRA
                    (0x5, 0x20) => {
                        self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64
                    }
                    // OR
                    (0x6, 0x00) => self.regs[rd] = self.regs[rs1] | self.regs[rs2],
                    // AND
                    (0x7, 0x00) => self.regs[rd] = self.regs[rs1] & self.regs[rs2],
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x} funct7 {:x}",
                            opcode, funct3, funct7
                        );
                        return Err(Exception::IllegalInstruction);
                    }
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
                    // DIVUW
                    (0x5, 0x01) => {
                        self.regs[rd] = match self.regs[rs2] {
                            0 => 0xffffffff_ffffffff,
                            _ => (self.regs[rs1] as u32).wrapping_div(self.regs[rs2] as u32) as i32
                                as u64,
                        };
                    }
                    // SRAW
                    (0x5, 0x20) => {
                        self.regs[rd] = ((self.regs[rs1] as i32) >> (shamt as i32)) as u64
                    }
                    // REMUW
                    (0x7, 0x01) => {
                        self.regs[rd] = match self.regs[rs2] {
                            0 => self.regs[rs1],
                            _ => (self.regs[rs1] as u32).wrapping_rem(self.regs[rs2] as u32) as i32
                                as u64,
                        };
                    }
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x} funct7 {:x}",
                            opcode, funct3, funct7
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            0x63 => {
                let imm = ((inst & 0x80000000) as i32 as i64 >> 19) as u64
                    | ((inst >> 20) & 0x7e0) as u64
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
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            // JALR
            0x67 => {
                self.regs[rd] = self.pc;

                let imm = ((((inst & 0xfff00000) as i32) as i64) >> 20) as u64;
                self.pc = (self.regs[rs1].wrapping_add(imm)) & !1;
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
            0x73 => {
                let address = ((inst & 0xfff00000) >> 20) as usize;
                match funct3 {
                    0x0 => {
                        match (rs2, funct7) {
                            // ECALL
                            (0x0, 0x0) => match self.mode {
                                Mode::User => {
                                    return Err(Exception::EnvironmentCallFromUMode);
                                }
                                Mode::Supervisor => {
                                    return Err(Exception::EnvironmentCallFromSMode);
                                }
                                Mode::Machine => {
                                    return Err(Exception::EnvironmentCallFromMMode);
                                }
                            },
                            // EBREAK
                            (0x1, 0x0) => {
                                return Err(Exception::Breakpoint);
                            }
                            // SRET
                            (0x2, 0x8) => {
                                self.pc = self.load_csr(SEPC);

                                let sstatus = self.load_csr(SSTATUS);
                                self.mode = match (sstatus >> 8) & 1 {
                                    1 => Mode::Supervisor,
                                    _ => Mode::User,
                                };

                                match (sstatus >> 5) & 1 {
                                    1 => self.store_csr(SSTATUS, sstatus | (1 << 1)),
                                    _ => self.store_csr(SSTATUS, sstatus & !(1 << 1)),
                                }
                                self.store_csr(SSTATUS, self.load_csr(SSTATUS) | (1 << 5));
                                self.store_csr(SSTATUS, self.load_csr(SSTATUS) & !(1 << 8));
                            }
                            // MRET
                            (0x2, 0x18) => {
                                self.pc = self.load_csr(MEPC);

                                let mstatus = self.load_csr(MSTATUS);
                                self.mode = match (mstatus >> 11) & 0b11 {
                                    2 => Mode::Machine,
                                    1 => Mode::Supervisor,
                                    _ => Mode::User,
                                };

                                match (mstatus >> 7) & 1 {
                                    1 => self.store_csr(MSTATUS, mstatus | (1 << 3)),
                                    _ => self.store_csr(MSTATUS, mstatus & !(1 << 3)),
                                }

                                self.store_csr(MSTATUS, self.load_csr(MSTATUS) | (1 << 7));
                                self.store_csr(MSTATUS, self.load_csr(MSTATUS) & !(3 << 11));
                            }
                            // SFENCE.VMA
                            (_, 0x9) => (),
                            _ => {
                                println!(
                                    "Unsupported instruction: opcode {:x} funct3 {:x} funct7 {:x}",
                                    opcode, funct3, funct7
                                );
                                return Err(Exception::IllegalInstruction);
                            }
                        }
                    }
                    // CSRRW
                    0x1 => {
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, self.regs[rs1]);
                    }
                    // CSRRS
                    0x2 => {
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, self.regs[rd] | self.regs[rs1]);
                    }
                    // CSRRC
                    0x3 => {
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, self.regs[rd] & !self.regs[rs1]);
                    }
                    // CSRRWI
                    0x5 => {
                        let uimm = rs1 as u64;
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, uimm);
                    }
                    // CSRRSI
                    0x6 => {
                        let uimm = rs1 as u64;
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, self.regs[rd] | uimm);
                    }
                    // CSRRCI
                    0x7 => {
                        let uimm = rs1 as u64;
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, self.regs[rd] & !uimm);
                    }
                    _ => {
                        println!(
                            "Unsupported instruction: opcode {:x} funct3 {:x}",
                            opcode, funct3
                        );
                        return Err(Exception::IllegalInstruction);
                    }
                }
            }
            _ => {
                println!("Unsupported instruction: opcode {:x}", opcode);
                return Err(Exception::IllegalInstruction);
            }
        }
        Ok(())
    }
}
