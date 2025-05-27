use crate::arch::x86_64::mm as arch_mm;
use crate::kernel::mm::{LinearAddr, PAGE_SIZE, PhysicalAddr};

pub fn memzero(addr: usize, size: usize) {
    for i in 0..size {
        unsafe { *((addr + i) as *mut u8) = 0 };
    }
}

pub fn addr_to_page_entries<'a>(addr: usize) -> &'a mut [usize] {
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
