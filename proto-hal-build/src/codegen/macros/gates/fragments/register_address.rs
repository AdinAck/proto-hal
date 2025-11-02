use ir::structures::{peripheral::Peripheral, register::Register};

/// The MMIO mapped address of the register.
pub fn register_address(peripheral: &Peripheral, register: &Register) -> u32 {
    peripheral.base_addr + register.offset
}
