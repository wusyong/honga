//! System bus contains memory & memory-mapped peripheral devices.

mod clint;
mod memory;
mod plic;
mod uart;
pub mod virtio;

pub use clint::{CLINT_BASE, CLINT_SIZE};
pub use memory::{MEMORY_BASE, MEMORY_SIZE};
pub use plic::{PLIC_BASE, PLIC_SCLAIM, PLIC_SIZE};
pub use uart::{UART_BASE, UART_IRQ, UART_SIZE};
pub use virtio::{VIRTIO_BASE, VIRTIO_IRQ, VIRTIO_SIZE};

use crate::exception::Exception;
use clint::Clint;
use memory::Memory;
use plic::Plic;
use uart::Uart;
use virtio::Virtio;

trait Device {
    fn load(&self, addr: u64, size: usize) -> Result<u64, Exception>;
    fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception>;
}

/// System bus.
pub struct Bus {
    clint: Clint,
    memory: Memory,
    plic: Plic,
    pub uart: Uart,
    pub virtio: Virtio,
}

impl Bus {
    pub fn new(binary: Vec<u8>, image: Vec<u8>) -> Bus {
        Self {
            memory: Memory::new(binary),
            clint: Clint::new(),
            plic: Plic::new(),
            uart: Uart::new(),
            virtio: Virtio::new(image),
        }
    }

    pub fn load(&self, addr: u64, size: usize) -> Result<u64, Exception> {
        if CLINT_BASE <= addr && addr < CLINT_BASE + CLINT_SIZE {
            return self.clint.load(addr, size);
        }
        if PLIC_BASE <= addr && addr < PLIC_BASE + PLIC_SIZE {
            return self.plic.load(addr, size);
        }
        if UART_BASE <= addr && addr < UART_BASE + UART_SIZE {
            return self.uart.load(addr, size);
        }
        if VIRTIO_BASE <= addr && addr < VIRTIO_BASE + VIRTIO_SIZE {
            return self.virtio.load(addr, size);
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
        if UART_BASE <= addr && addr < UART_BASE + UART_SIZE {
            return self.uart.store(addr, size, value);
        }
        if VIRTIO_BASE <= addr && addr < VIRTIO_BASE + VIRTIO_SIZE {
            return self.virtio.store(addr, size, value);
        }
        if MEMORY_BASE <= addr {
            return self.memory.store(addr, size, value);
        }
        Err(Exception::StoreAMOAccessFault)
    }
}
