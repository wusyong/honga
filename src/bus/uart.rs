//! The uart module contains the implementation of a universal asynchronous receiver-transmitter
//! (UART). The device is 16550a UART, which is used in the QEMU virt machine.
//! See the spec: http://byterunner.com/16550.html

#![allow(dead_code)]

use std::io;
use std::io::prelude::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
};
use std::thread;

use crate::bus::Device;
use crate::exception::*;

pub const UART_BASE: u64 = 0x1000_0000;
pub const UART_SIZE: u64 = 0x100;
/// Receive holding register (for input bytes).
pub const UART_RHR: u64 = UART_BASE + 0;
/// Transmit holding register (for output bytes).
pub const UART_THR: u64 = UART_BASE + 0;
/// Line control register.
pub const UART_LCR: u64 = UART_BASE + 3;
/// Line status register.
/// LSR BIT 0:
///     0 = no data in receive holding register or FIFO.
///     1 = data has been receive and saved in the receive holding register or FIFO.
/// LSR BIT 5:
///     0 = transmit holding register is full. 16550 will not accept any data for transmission.
///     1 = transmitter hold register (or FIFO) is empty. CPU can load the next character.
pub const UART_LSR: u64 = UART_BASE + 5;

/// The receiver (RX) bit.
pub const UART_LSR_RX: u8 = 1;
/// The transmitter (TX) bit.
pub const UART_LSR_TX: u8 = 1 << 5;
/// The interrupt request of UART.
pub const UART_IRQ: u64 = 10;

pub struct Uart {
    /// Pair of an array for UART buffer and a conditional variable.
    uart: Arc<(Mutex<[u8; UART_SIZE as usize]>, Condvar)>,
    /// Bit if an interrupt happens.
    interrupt: Arc<AtomicBool>,
}

impl Uart {
    pub fn new() -> Self {
        let uart = Arc::new((Mutex::new([0; UART_SIZE as usize]), Condvar::new()));
        let interrupt = Arc::new(AtomicBool::new(false));
        {
            let (uart, _) = &*uart;
            let mut uart = uart.lock().unwrap();
            uart[(UART_LSR - UART_BASE) as usize] |= UART_LSR_TX;
        }

        let mut byte = [0];
        let cloned_uart = uart.clone();
        let cloned_interrupt = interrupt.clone();
        let _uart_thread_for_read = thread::spawn(move || loop {
            match io::stdin().read(&mut byte) {
                Ok(_) => {
                    let (uart, cvar) = &*cloned_uart;
                    let mut uart = uart.lock().unwrap();
                    while (uart[(UART_LSR - UART_BASE) as usize] & UART_LSR_RX) == 1 {
                        uart = cvar.wait(uart).unwrap();
                    }

                    uart[0] = byte[0];
                    cloned_interrupt.store(true, Ordering::Release);
                    uart[(UART_LSR - UART_BASE) as usize] |= UART_LSR_RX;
                }
                Err(e) => eprintln!("{}", e),
            }
        });
        Self { uart, interrupt }
    }

    /// Return true if an interrupt is pending. Clear the flag by swapping a value.
    pub fn is_interrupting(&self) -> bool {
        self.interrupt.swap(false, Ordering::Acquire)
    }
}

impl Device for Uart {
    fn load(&self, addr: u64, size: usize) -> Result<u64, Exception> {
        match size {
            8 => {
                let (uart, cvar) = &*self.uart;
                let mut uart = uart.lock().unwrap();
                match addr {
                    UART_RHR => {
                        cvar.notify_one();
                        uart[(UART_LSR - UART_BASE) as usize] &= !UART_LSR_RX;
                        Ok(uart[(UART_RHR - UART_BASE) as usize] as u64)
                    }
                    _ => Ok(uart[(addr - UART_BASE) as usize] as u64),
                }
            }
            _ => Err(Exception::LoadAccessFault),
        }
    }

    fn store(&mut self, addr: u64, size: usize, value: u64) -> Result<(), Exception> {
        match size {
            8 => {
                let (uart, _) = &*self.uart;
                let mut uart = uart.lock().unwrap();
                match addr {
                    UART_THR => {
                        print!("{}", value as u8 as char);
                        io::stdout().flush().expect("failed to flush stdout");
                    }
                    _ => uart[(addr - UART_BASE) as usize] = value as u8,
                }
                Ok(())
            }
            _ => Err(Exception::StoreAMOAccessFault),
        }
    }
}
