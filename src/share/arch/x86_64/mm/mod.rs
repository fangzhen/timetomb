use crate::arch::x86_64::ffi_shared;
use crate::arch::x86_64::mm as arch_mm;
use crate::kernel::mm::{LinearAddr, PhysicalAddr, PAGE_SIZE};
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
        let entries = addr_to_page_entries(laddr);
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

pub fn memzero(addr: usize, size: usize) {
    for i in 0..size {
        unsafe { *((addr + i) as *mut u8) = 0 };
    }
}

pub fn addr_to_page_entries<'a>(addr: LinearAddr) -> &'a mut [usize] {
    unsafe {
        core::slice::from_raw_parts_mut(
            addr as *mut usize,
            PAGE_SIZE / core::mem::size_of::<usize>(),
        )
    }
}

fn add_page_entry(
    pgt_allocator: &mut impl FnMut() -> PhysicalAddr,
    entries_addr: LinearAddr,
    idx: usize,
    flags: usize,
) -> PhysicalAddr {
    let entries = addr_to_page_entries(entries_addr);
    if entries[idx] & arch_mm::PAGE_BIT_P_PRESENT == 0 {
        let addr = pgt_allocator();
        entries[idx] = addr & arch_mm::PAGE_ADDR_MASK | flags;
    }
    return entries[idx];
}

/// Add page table for physical -> linear mapping
/// p2l: physical -> linear addr mapping in current page table
pub fn add_page_mapping(
    pgt_allocator: &mut impl FnMut() -> PhysicalAddr,
    p2l: fn(PhysicalAddr) -> LinearAddr,
    linear: LinearAddr,
    physical: PhysicalAddr,
    pml4_addr: PhysicalAddr,
) {
    // flags: page is present, user readable and writable
    // TODO(fangzhen) flags should be specified.
    let flags =
        arch_mm::PAGE_BIT_P_PRESENT | arch_mm::PAGE_BIT_RW_WRITABLE | arch_mm::PAGE_BIT_US_USER;
    /* extract mapping table indices from virtual address */
    let pml4_idx = (linear >> 39) & 0x1ff;
    let pml3_idx = (linear >> 30) & 0x1ff; //pdp
    let pml2_idx = (linear >> 21) & 0x1ff; //pd
    let pml1_idx = (linear >> 12) & 0x1ff; //pt

    let pml4_addr = p2l(pml4_addr);
    let pml4_entry = add_page_entry(pgt_allocator, pml4_addr, pml4_idx, flags);
    let pml3_addr = p2l(pml4_entry & arch_mm::PAGE_ADDR_MASK);
    let pml3_entry = add_page_entry(pgt_allocator, pml3_addr, pml3_idx, flags);
    let pml2_addr = p2l(pml3_entry & arch_mm::PAGE_ADDR_MASK);
    let pml2_entry = add_page_entry(pgt_allocator, pml2_addr, pml2_idx, flags);
    let pml1_addr = p2l(pml2_entry & arch_mm::PAGE_ADDR_MASK);
    let pml1_entry = addr_to_page_entries(pml1_addr);
    pml1_entry[pml1_idx] = physical & arch_mm::PAGE_ADDR_MASK | flags;
}
