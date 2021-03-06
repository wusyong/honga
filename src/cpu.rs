use crate::bus::{
    virtio::Virtio, Bus, MEMORY_BASE, MEMORY_SIZE, PLIC_SCLAIM, UART_IRQ, VIRTIO_IRQ,
};
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

/// The page size (4 KiB) for the virtual memory system.
const PAGE_SIZE: u64 = 4096;

/// Privileged mode.
#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum Mode {
    User = 0,
    Supervisor = 1,
    Machine = 3,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum AccessType {
    Instruction,
    Load,
    Store,
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
    /// SV39 paging flag.
    pub enable_paging: bool,
    /// physical page number (PPN) × PAGE_SIZE (4096).
    pub page_table: u64,
}

impl Cpu {
    /// Create a new `Cpu` object.
    pub fn new(binary: Vec<u8>, image: Vec<u8>) -> Self {
        let mut regs = [0; 32];
        // Set the register x2 with the size of a memory when a CPU is instantiated.
        regs[2] = MEMORY_SIZE + MEMORY_BASE;

        Self {
            regs,
            pc: MEMORY_BASE,
            bus: Bus::new(binary, image),
            csr: [0; 4096],
            mode: Mode::Machine,
            enable_paging: false,
            page_table: 0,
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
        } else if self.bus.virtio.is_interrupting() {
            // Access disk by direct memory access (DMA). An interrupt is raised after a disk
            // access is done.
            Virtio::disk_access(self);
            irq = VIRTIO_IRQ;
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

    fn update_paging(&mut self, csr_addr: usize) {
        if csr_addr != SATP {
            return;
        }

        self.page_table = (self.load_csr(SATP) & ((1 << 44) - 1)) * PAGE_SIZE;
        let mode = self.load_csr(SATP) >> 60;
        if mode == 8 {
            self.enable_paging = true;
        } else {
            self.enable_paging = false;
        }
    }

    fn translate(&mut self, addr: u64, access_type: AccessType) -> Result<u64, Exception> {
        if !self.enable_paging {
            return Ok(addr);
        }

        // The following comments are cited from 4.3.2 Virtual Address Translation Process
        // in "The RISC-V Instruction Set Manual Volume II-Privileged Architecture_20190608".

        // "A virtual address va is translated into a physical address pa as follows:"
        let levels = 3;
        let vpn = [
            (addr >> 12) & 0x1ff,
            (addr >> 21) & 0x1ff,
            (addr >> 30) & 0x1ff,
        ];

        // "1. Let a be satp.ppn × PAGESIZE, and let i = LEVELS − 1. (For Sv32, PAGESIZE=212
        //     and LEVELS=2.)"
        let mut a = self.page_table;
        let mut i: i64 = levels - 1;
        let mut pte;
        loop {
            // "2. Let pte be the value of the PTE at address a+va.vpn[i]×PTESIZE. (For Sv32,
            //     PTESIZE=4.) If accessing pte violates a PMA or PMP check, raise an access
            //     exception corresponding to the original access type."
            pte = self.bus.load(a + vpn[i as usize] * 8, 64)?;

            // "3. If pte.v = 0, or if pte.r = 0 and pte.w = 1, stop and raise a page-fault
            //     exception corresponding to the original access type."
            let v = pte & 1;
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;
            if v == 0 || (r == 0 && w == 1) {
                match access_type {
                    AccessType::Instruction => return Err(Exception::InstructionPageFault),
                    AccessType::Load => return Err(Exception::LoadPageFault),
                    AccessType::Store => return Err(Exception::StoreAMOPageFault),
                }
            }

            // "4. Otherwise, the PTE is valid. If pte.r = 1 or pte.x = 1, go to step 5.
            //     Otherwise, this PTE is a pointer to the next level of the page table.
            //     Let i = i − 1. If i < 0, stop and raise a page-fault exception
            //     corresponding to the original access type. Otherwise,
            //     let a = pte.ppn × PAGESIZE and go to step 2."
            if r == 1 || x == 1 {
                break;
            }
            i -= 1;
            let ppn = (pte >> 10) & 0x0fff_ffff_ffff;
            a = ppn * PAGE_SIZE;
            if i < 0 {
                match access_type {
                    AccessType::Instruction => return Err(Exception::InstructionPageFault),
                    AccessType::Load => return Err(Exception::LoadPageFault),
                    AccessType::Store => return Err(Exception::StoreAMOPageFault),
                }
            }
        }

        // A leaf PTE has been found.
        let ppn = [
            (pte >> 10) & 0x1ff,
            (pte >> 19) & 0x1ff,
            (pte >> 28) & 0x03ff_ffff,
        ];

        // We skip implementing from step 5 to 7.

        // "5. A leaf PTE has been found. Determine if the requested dram access is allowed by
        //     the pte.r, pte.w, pte.x, and pte.u bits, given the current privilege mode and the
        //     value of the SUM and MXR fields of the mstatus register. If not, stop and raise a
        //     page-fault exception corresponding to the original access type."

        // "6. If i > 0 and pte.ppn[i − 1 : 0] ̸= 0, this is a misaligned superpage; stop and
        //     raise a page-fault exception corresponding to the original access type."

        // "7. If pte.a = 0, or if the dram access is a store and pte.d = 0, either raise a
        //     page-fault exception corresponding to the original access type, or:
        //     • Set pte.a to 1 and, if the dram access is a store, also set pte.d to 1.
        //     • If this access violates a PMA or PMP check, raise an access exception
        //     corresponding to the original access type.
        //     • This update and the loading of pte in step 2 must be atomic; in particular, no
        //     intervening store to the PTE may be perceived to have occurred in-between."

        // "8. The translation is successful. The translated physical address is given as
        //     follows:
        //     • pa.pgoff = va.pgoff.
        //     • If i > 0, then this is a superpage translation and pa.ppn[i−1:0] =
        //     va.vpn[i−1:0].
        //     • pa.ppn[LEVELS−1:i] = pte.ppn[LEVELS−1:i]."
        let offset = addr & 0xfff;
        match i {
            0 => {
                let ppn = (pte >> 10) & 0x0fff_ffff_ffff;
                Ok((ppn << 12) | offset)
            }
            1 => {
                // Superpage translation. A superpage is a dram page of larger size than an
                // ordinary page (4 KiB). It reduces TLB misses and improves performance.
                Ok((ppn[2] << 30) | (ppn[1] << 21) | (vpn[0] << 12) | offset)
            }
            2 => {
                // Superpage translation. A superpage is a dram page of larger size than an
                // ordinary page (4 KiB). It reduces TLB misses and improves performance.
                Ok((ppn[2] << 30) | (vpn[1] << 21) | (vpn[0] << 12) | offset)
            }
            _ => match access_type {
                AccessType::Instruction => return Err(Exception::InstructionPageFault),
                AccessType::Load => return Err(Exception::LoadPageFault),
                AccessType::Store => return Err(Exception::StoreAMOPageFault),
            },
        }
    }

    /// Load a value from a memory.
    pub fn load(&mut self, addr: u64, size: usize) -> Result<u64, Exception> {
        let p_addr = self.translate(addr, AccessType::Load)?;
        self.bus.load(p_addr, size)
    }

    /// Store a value to a memory.
    pub fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception> {
        let p_addr = self.translate(addr, AccessType::Store)?;
        self.bus.store(p_addr, size, value)
    }

    /// Fetch the instruction from memory.
    pub fn fetch(&mut self) -> Result<u32, Exception> {
        let p_pc = self.translate(self.pc, AccessType::Instruction)?;
        match self.bus.load(p_pc, 32) {
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
                        let value = self.load(address, 8)?;
                        self.regs[rd] = value as i8 as i64 as u64;
                    }
                    // LH
                    0x1 => {
                        let value = self.load(address, 16)?;
                        self.regs[rd] = value as i16 as i64 as u64;
                    }
                    // LW
                    0x2 => {
                        let value = self.load(address, 32)?;
                        self.regs[rd] = value as i32 as i64 as u64;
                    }
                    // LD
                    0x3 => {
                        let value = self.load(address, 64)?;
                        self.regs[rd] = value as i64 as u64;
                    }
                    // LBU
                    0x4 => {
                        let value = self.load(address, 8)?;
                        self.regs[rd] = value;
                    }
                    // LHU
                    0x5 => {
                        let value = self.load(address, 16)?;
                        self.regs[rd] = value;
                    }
                    // LWU
                    0x6 => {
                        let value = self.load(address, 32)?;
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
            0x0f => {
                // A fence instruction does nothing because this emulator executes an
                // instruction sequentially on a single thread.
                match funct3 {
                    0x0 => {} // fence
                    _ => {
                        println!(
                            "not implemented yet: opcode {:#x} funct3 {:#x}",
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
                    0x5 => match funct7 >> 1 {
                        // SRLI
                        0x00 => self.regs[rd] = self.regs[rs1].wrapping_shr(shamt),
                        // SRAI
                        0x10 => self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64,
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
                    0x0 => self.store(address, 8, self.regs[rs2])?,
                    // SH
                    0x1 => self.store(address, 16, self.regs[rs2])?,
                    // SW
                    0x2 => self.store(address, 32, self.regs[rs2])?,
                    // SD
                    0x3 => self.store(address, 64, self.regs[rs2])?,
                    _ => (),
                }
            }
            // RV64A: "A" standard extension for atomic instructions
            0x2f => {
                let funct5 = (funct7 & 0x7c) >> 2;
                let _aq = (funct7 & 0x02) >> 1;
                let _rl = funct7 & 0x01;

                match (funct3, funct5) {
                    // AMOADD.W
                    (0x2, 0x00) => {
                        let tmp = self.load(self.regs[rs1], 32)?;
                        self.store(
                            self.regs[rs1],
                            32,
                            tmp.wrapping_add(self.regs[rs2]),
                        )?;
                        self.regs[rd] = tmp;
                    }
                    // AMOADD.D
                    (0x3, 0x00) => {
                        let tmp = self.load(self.regs[rs1], 64)?;
                        self.store(
                            self.regs[rs1],
                            64,
                            tmp.wrapping_add(self.regs[rs2]),
                        )?;
                        self.regs[rd] = tmp;
                    }
                    // AMOSWAP.W
                    (0x2, 0x01) => {
                        let tmp = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, self.regs[rs2])?;
                        self.regs[rd] = tmp;
                    }
                    // AMOSWAP.D
                    (0x3, 0x01) => {
                        let tmp = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2])?;
                        self.regs[rd] = tmp;
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
                    (0x0, 0x01) => self.regs[rd] = self.regs[rs1].wrapping_mul(self.regs[rs2]),
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
                let tmp = self.pc;
                let imm = ((((inst & 0xfff00000) as i32) as i64) >> 20) as u64;
                self.pc = (self.regs[rs1].wrapping_add(imm)) & !1;
                self.regs[rd] = tmp;
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
                        let tmp = self.load_csr(address);
                        self.store_csr(address, self.regs[rs1]);
                        self.regs[rd] = tmp;
                        self.update_paging(address);
                    }
                    // CSRRS
                    0x2 => {
                        let tmp = self.load_csr(address);
                        self.store_csr(address, tmp | self.regs[rs1]);
                        self.regs[rd] = tmp;
                        self.update_paging(address);
                    }
                    // CSRRC
                    0x3 => {
                        let tmp = self.load_csr(address);
                        self.store_csr(address, tmp & !self.regs[rs1]);
                        self.regs[rd] = tmp;
                        self.update_paging(address);
                    }
                    // CSRRWI
                    0x5 => {
                        let uimm = rs1 as u64;
                        self.regs[rd] = self.load_csr(address);
                        self.store_csr(address, uimm);
                        self.update_paging(address);
                    }
                    // CSRRSI
                    0x6 => {
                        let uimm = rs1 as u64;
                        let tmp = self.load_csr(address);
                        self.store_csr(address, tmp | uimm);
                        self.regs[rd] = tmp;
                        self.update_paging(address);
                    }
                    // CSRRCI
                    0x7 => {
                        let uimm = rs1 as u64;
                        let tmp = self.load_csr(address);
                        self.store_csr(address, tmp & !uimm);
                        self.regs[rd] = tmp;
                        self.update_paging(address);
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
