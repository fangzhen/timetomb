pub mod init;

use crate::arch::x86_64::ffi_shared;
use crate::kernel::mm::PAGE_SIZE;
use crate::kernel::mm::{LinearAddr, PhysicalAddr};
pub const VMKERNEL_ENTRY_ADDRESS: usize = ffi_shared::VMKERNEL_ENTRY_ADDRESS;
/* page entry bitflags */
const PAGE_BIT_P_PRESENT: usize = 1 << 0;
const PAGE_BIT_RW_WRITABLE: usize = 1 << 1;
const PAGE_BIT_US_USER: usize = 1 << 2;

/* bit mask for page aligned 52-bit address */
pub const PAGE_ADDR_MASK: usize = 0x000ffffffffff000;

// MemoryType and MemoryDescriptor is defined as uefi spec now.
#[allow(dead_code)]
#[derive(PartialEq, Debug, Copy, Clone)]
#[repr(C)]
pub enum MemoryType {
    EfiReservedMemoryType,
    EfiLoaderCode,
    EfiLoaderData,
    EfiBootServicesCode,
    EfiBootServicesData,
    EfiRuntimeServicesCode,
    EfiRuntimeServicesData,
    EfiConventionalMemory,
    EfiUnusableMemory,
    EfiACPIReclaimMemory,
    EfiACPIMemoryNVS,
    EfiMemoryMappedIO,
    EfiMemoryMappedIOPortSpace,
    EfiPalCode,
    EfiPersistentMemory,
    EfiMaxMemoryType,
}
pub type MemoryAttribute = u64;
/// A structure describing a region of memory.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct MemoryDescriptor {
    /// Type of memory occupying this range.
    pub mem_type: MemoryType,
    /// Starting physical address.
    pub phys_start: usize,
    /// Starting virtual address.
    pub virt_start: usize,
    /// Number of 4 KiB pages contained in this range.
    pub page_count: usize,
    /// The capability attributes of this memory range.
    pub att: MemoryAttribute,
}

pub fn print_physical_map(map: &[MemoryDescriptor]) {
    for (_, &d) in map.iter().enumerate() {
        log::info!(
            "UEFI memorymap. Type: {:?} PhysicalStart: {:#x} PhysicalEnd: {:#x} VirtualStart: {:#x} Pages: {} Attribute: {}",
            d.mem_type,
            d.phys_start,
            d.phys_start + d.page_count * PAGE_SIZE,
            d.virt_start,
            d.page_count,
            d.att,
        );
    }
}

pub fn print_pagetable_chain(
    p2l: fn(PhysicalAddr) -> LinearAddr,
    linear: LinearAddr,
    cr3_addr: PhysicalAddr,
) {
    log::info!("Print page table entries for address: {:#x}", linear);
    let pml4_idx = (linear >> 39) & 0x1ff;
    let pml3_idx = (linear >> 30) & 0x1ff; //pdp
    let pml2_idx = (linear >> 21) & 0x1ff; //pd
    let pml1_idx = (linear >> 12) & 0x1ff; //pt

    fn print_entry(
        p2l: fn(PhysicalAddr) -> LinearAddr,
        addr: PhysicalAddr,
        idx: usize,
    ) -> PhysicalAddr {
        let laddr = p2l(addr);
        let entries = init::addr_to_page_entries(laddr);
        let entry = entries[idx];
        log::info!(
            "entry: {:#x}, address: {:#x}",
            entry,
            entry & PAGE_ADDR_MASK
        );
        return entry & PAGE_ADDR_MASK;
    }

    let pml3_addr = print_entry(p2l, cr3_addr, pml4_idx);
    let pml2_addr = print_entry(p2l, pml3_addr, pml3_idx);
    let pml1_addr = print_entry(p2l, pml2_addr, pml2_idx);
    let _phy_addr = print_entry(p2l, pml1_addr, pml1_idx);
}
