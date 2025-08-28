use crate::kernel::mm::PhysicalAddr;

pub mod ffi_shared;
pub mod mm;
pub mod uart;

#[repr(C, packed(2))]
pub struct DescriptorTablePointer {
    pub limit: u16,
    pub base: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SetupHeader {
    // used memory regions
    pub mem_desc_count: usize,
    pub mem_desc: PhysicalAddr,
    pub cr3_addr: usize,
    pub pgtable_size: usize,
    pub identity_map_max_idx: usize,
    pub kernel_physical: usize,
    pub kernel_size: usize,
    pub kernel_stack_physical: usize,
    pub kernel_area_size: usize,
    pub rsdp_addr: usize,
}
