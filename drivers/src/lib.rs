#![no_std]

#[cfg(target_arch = "arm")]
pub use drivers_hw::*;
#[cfg(not(target_arch = "arm"))]
pub use drivers_simu::*;
