pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_STACK_SIZE: usize = PAGE_SIZE * 32;
pub type PhysicalAddr = usize;
pub type LinearAddr = usize;

pub const PAGE_FLAG_PHYSICAL: u32 = 0x01; // Physical page is normal memory to use
