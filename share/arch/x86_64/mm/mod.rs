pub mod init;

use crate::arch::x86_64::ffi_shared;
use crate::kernel::mm::PAGE_SIZE;
use crate::kernel::mm::{self, LinearAddr, PhysicalAddr};
pub const VMKERNEL_ENTRY_ADDRESS: usize = ffi_shared::VMKERNEL_ENTRY_ADDRESS;
pub const P2L_OFFSET_BASE: usize = 0xffff888000000000;
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
pub fn p2l(physical: PhysicalAddr) -> LinearAddr {
    return physical + P2L_OFFSET_BASE;
}

pub fn l2p(linear: LinearAddr) -> PhysicalAddr {
    return linear - P2L_OFFSET_BASE;
}

pub fn print_memory_map(map: &[MemoryDescriptor]) {
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

pub fn generate_memblock_from_uefi_map(uefi_map: &[MemoryDescriptor]) {
    for d in uefi_map {
        if d.mem_type != MemoryType::EfiReservedMemoryType {
            // TODO(fangzhen) EfiReservedMemoryType uses physical address with high address
            // in qemu
            let flag = mm::PAGE_FLAG_PHYSICAL;
            mm::memblock::add_memory(d.phys_start, d.page_count * PAGE_SIZE, flag);
            if d.mem_type != MemoryType::EfiConventionalMemory {
                // Simple set all memory except EfiConventionalMemory as used.
                // e.g. kernel itself is loaded by EFI firmware with type EfiLoaderCode.
                // UEFI system table resides in EfiRuntimeServicesData.
                mm::memblock::add_used_memory(d.phys_start, d.page_count * PAGE_SIZE, flag);
            } else if d.phys_start == 0 {
                // TODO(fangzhen) mark address 0 as allocated to avoid allocate later.
                mm::memblock::add_used_memory(0, PAGE_SIZE, flag);
            }
        }
    }
}

pub fn print_pagetable_chain(linear: LinearAddr, cr3_addr: PhysicalAddr) {
    log::info!("Print page table entries for address: {:#x}", linear);
    let pml4_idx = (linear >> 39) & 0x1ff;
    let pml3_idx = (linear >> 30) & 0x1ff; //pdp
    let pml2_idx = (linear >> 21) & 0x1ff; //pd
    let pml1_idx = (linear >> 12) & 0x1ff; //pt

    fn print_entry(addr: PhysicalAddr, idx: usize) -> PhysicalAddr {
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

    let pml3_addr = print_entry(cr3_addr, pml4_idx);
    let pml2_addr = print_entry(pml3_addr, pml3_idx);
    let pml1_addr = print_entry(pml2_addr, pml2_idx);
    let _phy_addr = print_entry(pml1_addr, pml1_idx);
}
