//! System bus contains memory & memory-mapped peripheral devices.

mod clint;
mod memory;
mod plic;

pub use clint::{CLINT_BASE, CLINT_SIZE};
pub use memory::{MEMORY_BASE, MEMORY_SIZE};
pub use plic::{PLIC_BASE, PLIC_SIZE};

use crate::exception::Exception;
use clint::Clint;
use memory::Memory;
use plic::Plic;

trait Device {
    fn load(&self, addr: u64, size: usize) -> Result<u64, Exception>;
    fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception>;
}

/// System bus.
pub struct Bus {
    clint: Clint,
    memory: Memory,
    plic: Plic,
}

impl Bus {
    pub fn new(binary: Vec<u8>) -> Bus {
        Self {
            memory: Memory::new(binary),
            clint: Clint::new(),
            plic: Plic::new(),
        }
    }

    pub fn load(&self, addr: u64, size: usize) -> Result<u64, Exception> {
        if CLINT_BASE <= addr && addr < CLINT_BASE + CLINT_SIZE {
            return self.clint.load(addr, size);
        }
        if PLIC_BASE <= addr && addr < PLIC_BASE + PLIC_SIZE {
            return self.plic.load(addr, size);
        }
        if MEMORY_BASE <= addr {
            return self.memory.load(addr, size);
        }
        Err(Exception::LoadAccessFault)
    }

    pub fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception> {
        if CLINT_BASE <= addr && addr < CLINT_BASE + CLINT_SIZE {
            return self.clint.store(addr, size, value);
        }
        if PLIC_BASE <= addr && addr < PLIC_BASE + PLIC_SIZE {
            return self.plic.store(addr, size, value);
        }
        if MEMORY_BASE <= addr {
            return self.memory.store(addr, size, value);
        }
        Err(Exception::StoreAMOAccessFault)
    }
}
