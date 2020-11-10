use crate::exception::Exception;

/// Set memory size to 128MiB.
pub const MEMORY_SIZE: u64 = 128 * 1024 * 1024;
/// Address where QEMU virtual machine memory starts.
pub const MEMORY_BASE: u64 = 0x8000_0000;

/// Random-access memory.
pub struct Memory(pub Vec<u8>);

impl Memory {
    /// Create `Memory` with fixed memory size.
    pub fn new(binary: Vec<u8>) -> Self {
        let mut memory = vec![0u8; MEMORY_SIZE as usize];
        memory.splice(..binary.len(), binary);
        Self(memory)
    }

    /// Load bytes with requested size from little-endian memory.
    pub fn load(&self, address: u64, size: usize) -> Result<u64, Exception> {
        match size {
            8 => Ok(self.load_8bits(address)),
            16 => Ok(self.load_16bits(address)),
            32 => Ok(self.load_32bits(address)),
            64 => Ok(self.load_64bits(address)),
            _ => Err(Exception::LoadAddressMisaligned),
        }
    }

    /// Store bytes with requested size to little-endian memory.
    pub fn store(&mut self, address: u64, size: usize, value: u64) -> Result<(), Exception> {
        match size {
            8 => Ok(self.store_8bits(address, value)),
            16 => Ok(self.store_16bits(address, value)),
            32 => Ok(self.store_32bits(address, value)),
            64 => Ok(self.store_64bits(address, value)),
            _ => Err(Exception::LoadAddressMisaligned),
        }
    }

    fn load_8bits(&self, address: u64) -> u64 {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] as u64
    }

    fn load_16bits(&self, address: u64) -> u64 {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] as u64 | ((self.0[index + 1] as u64) << 8)
    }

    fn load_32bits(&self, address: u64) -> u64 {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] as u64
            | ((self.0[index + 1] as u64) << 8)
            | ((self.0[index + 2] as u64) << 16)
            | ((self.0[index + 3] as u64) << 24)
    }

    fn load_64bits(&self, address: u64) -> u64 {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] as u64
            | ((self.0[index + 1] as u64) << 8)
            | ((self.0[index + 2] as u64) << 16)
            | ((self.0[index + 3] as u64) << 24)
            | ((self.0[index + 4] as u64) << 32)
            | ((self.0[index + 5] as u64) << 40)
            | ((self.0[index + 6] as u64) << 48)
            | ((self.0[index + 7] as u64) << 56)
    }

    fn store_8bits(&mut self, address: u64, value: u64) {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] = value as u8;
    }

    fn store_16bits(&mut self, address: u64, value: u64) {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] = (value & 0xff) as u8;
        self.0[index + 1] = ((value >> 8) & 0xff) as u8;
    }

    fn store_32bits(&mut self, address: u64, value: u64) {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] = (value & 0xff) as u8;
        self.0[index + 1] = ((value >> 8) & 0xff) as u8;
        self.0[index + 2] = ((value >> 16) & 0xff) as u8;
        self.0[index + 3] = ((value >> 24) & 0xff) as u8;
    }

    fn store_64bits(&mut self, address: u64, value: u64) {
        let index = (address - MEMORY_BASE) as usize;
        self.0[index] = (value & 0xff) as u8;
        self.0[index + 1] = ((value >> 8) & 0xff) as u8;
        self.0[index + 2] = ((value >> 16) & 0xff) as u8;
        self.0[index + 3] = ((value >> 24) & 0xff) as u8;
        self.0[index + 4] = ((value >> 32) & 0xff) as u8;
        self.0[index + 5] = ((value >> 40) & 0xff) as u8;
        self.0[index + 6] = ((value >> 48) & 0xff) as u8;
        self.0[index + 7] = ((value >> 56) & 0xff) as u8;
    }
}
