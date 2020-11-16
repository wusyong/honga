//! Interrupt module.

#![allow(dead_code)]

use crate::cpu::*;
use crate::csr::*;

/// Interrupt is an external asynchronous event that may cause
/// a hardware thread to experience an unexpected transfer of
/// control.
#[repr(u64)]
#[derive(Debug, Clone, Copy)]
pub enum Interrupt {
    UserSoftwareInterrupt = 0,
    SupervisorSoftwareInterrupt = 1,
    MachineSoftwareInterrupt = 3,
    UserTimerInterrupt = 4,
    SupervisorTimerInterrupt = 5,
    MachineTimerInterrupt = 7,
    UserExternalInterrupt = 8,
    SupervisorExternalInterrupt = 9,
    MachineExternalInterrupt = 11,
}

impl Interrupt {
    /// Handle trap from current exception.
    pub fn get_trap(&self, cpu: &mut Cpu) {
        let exception_pc = cpu.pc.wrapping_sub(4);
        let previous_mode = cpu.mode;

        // Set the interrupt bit.
        let cause = *self as u64 | (1 << 63);
        if (previous_mode as u8 <= Mode::Supervisor as u8)
            && ((cpu.load_csr(MEDELEG).wrapping_shr(cause as u32)) & 1 != 0)
        {
            // Handle the trap in S mode.
            cpu.mode = Mode::Supervisor;

            // Set the program counter to STVEC.
            let vector = match cpu.load_csr(STVEC) & 1 {
                1 => 4 * cause, // vectored mode
                _ => 0,         // direct mode
            };
            cpu.pc = (cpu.load_csr(STVEC) & !1) + vector;

            cpu.store_csr(SEPC, exception_pc & !1);
            cpu.store_csr(SCAUSE, cause);
            cpu.store_csr(STVAL, 0);
            cpu.store_csr(
                SSTATUS,
                if ((cpu.load_csr(SSTATUS) >> 1) & 1) == 1 {
                    cpu.load_csr(SSTATUS) | (1 << 5)
                } else {
                    cpu.load_csr(SSTATUS) & !(1 << 5)
                },
            );
            cpu.store_csr(SSTATUS, cpu.load_csr(SSTATUS) & !(1 << 1));
            match previous_mode {
                Mode::User => cpu.store_csr(SSTATUS, cpu.load_csr(SSTATUS) & !(1 << 8)),
                _ => cpu.store_csr(SSTATUS, cpu.load_csr(SSTATUS) | (1 << 8)),
            }
        } else {
            // Handle the trap in M mode.
            cpu.mode = Mode::Machine;

            // Set the program counter to MTVEC.
            let vector = match cpu.load_csr(MTVEC) & 1 {
                1 => 4 * cause, // vectored mode
                _ => 0,         // direct mode
            };
            cpu.pc = (cpu.load_csr(MTVEC) & !1) + vector;

            cpu.store_csr(MEPC, exception_pc & !1);
            cpu.store_csr(MCAUSE, cause);
            cpu.store_csr(MTVAL, 0);
            cpu.store_csr(
                MSTATUS,
                if ((cpu.load_csr(MSTATUS) >> 3) & 1) == 1 {
                    cpu.load_csr(MSTATUS) | (1 << 7)
                } else {
                    cpu.load_csr(MSTATUS) & !(1 << 7)
                },
            );
            cpu.store_csr(MSTATUS, cpu.load_csr(MSTATUS) & !(1 << 3));
            cpu.store_csr(MSTATUS, cpu.load_csr(MSTATUS) & !(0b11 << 11));
        }
    }
}
