//! System bus contains memory & memory-mapped peripheral devices.

mod memory;

pub use memory::{MEMORY_BASE, MEMORY_SIZE};

use crate::exception::Exception;
use memory::Memory;

trait Device {
    fn load(&self, addr: u64, size: usize) -> Result<u64, Exception>;
    fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception>;
}

/// System bus.
pub struct Bus {
    memory: Memory,
}

impl Bus {
    pub fn new(binary: Vec<u8>) -> Bus {
        Self {
            memory: Memory::new(binary),
        }
    }

    pub fn load(&self, addr: u64, size: usize) -> Result<u64, Exception> {
        if MEMORY_BASE <= addr {
            return self.memory.load(addr, size);
        }
        Err(Exception::LoadAccessFault)
    }

    pub fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception> {
        if MEMORY_BASE <= addr {
            return self.memory.store(addr, size, value);
        }
        Err(Exception::StoreAMOAccessFault)
    }
}
