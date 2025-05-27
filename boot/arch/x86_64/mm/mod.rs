use log::*;
use timetomb::arch::x86_64::mm as arch_mm;
use timetomb::kernel::mm::{memblock, LinearAddr, PhysicalAddr, PAGE_SIZE};

#[derive(Debug)]
pub struct PgtMemory {
    pub max: usize,
    pub current: usize,
}

static mut CR3_ADDR: usize = 0;
pub static mut PGT_MEMORY: PgtMemory = PgtMemory { max: 0, current: 0 };

fn p2l_before_init(physical: PhysicalAddr) -> LinearAddr {
    return physical;
}

// allocate physical page for page tables.
fn allocate_page_table(pages: &mut PgtMemory) -> PhysicalAddr {
    let physical_addr = pages.current;
    pages.current += PAGE_SIZE;
    if pages.current > pages.max {
        info!(
            "Not enough memory for page table. Probably Bugs. {:?}",
            pages
        );
        panic!()
    }
    arch_mm::init::memzero(p2l_before_init(physical_addr), PAGE_SIZE);
    return physical_addr;
}

// calulate max pages needed for page tabel to map linear address 0..max_addr
fn max_pt_pages(max_addr: usize) -> usize {
    return 4 + (max_addr >> 39) + (max_addr >> 30) + (max_addr >> 21);
}

pub fn init_paging() -> (PhysicalAddr, usize) {
    let max_physical = memblock::get_max_addr(unsafe { &*(&raw const memblock::ALL_MEMBLOCKS) });
    let size = max_pt_pages(max_physical) * PAGE_SIZE + 16 * PAGE_SIZE; // TODO: hardcode 16 pages for extra paging tables (kernel text for now)
    let pgt_addr = memblock::allocate_physical_memory(0, size, PAGE_SIZE, 0);
    unsafe {
        PGT_MEMORY.current = pgt_addr;
        PGT_MEMORY.max = size + pgt_addr;
        let cr3_addr = allocate_page_table(&mut *(&raw mut PGT_MEMORY));
        CR3_ADDR = cr3_addr;
        paging_direct_map(
            || allocate_page_table(&mut *(&raw mut PGT_MEMORY)),
            max_physical,
            cr3_addr,
        );
    }

    // TODO(fangzhen) only map used memory.
    let max_used = memblock::get_max_addr(unsafe { &*(&raw const memblock::USED_MEMBLOCKS) });
    let size = max_pt_pages(max_used) * PAGE_SIZE;
    let pgt_addr = memblock::allocate_physical_memory(0, size, PAGE_SIZE, 0);
    let mut pgt_pages = PgtMemory {
        current: pgt_addr,
        max: size + pgt_addr,
    };
    unsafe {
        let idx_max =
            paging_identity_map(|| allocate_page_table(&mut pgt_pages), max_used, CR3_ADDR);
        return (CR3_ADDR, idx_max);
    }
}

fn paging_identity_map(
    mut pgt_allocator: impl FnMut() -> PhysicalAddr,
    max_addr: PhysicalAddr,
    cr3_addr: PhysicalAddr,
) -> usize {
    for addr in (0..max_addr).step_by(PAGE_SIZE) {
        arch_mm::init::add_page_mapping(&mut pgt_allocator, p2l_before_init, addr, addr, cr3_addr);
    }
    return max_addr >> 39; // max index in pml4
}

// Mapping whole physical memory to virtual address with offset P2L_OFFSET_BASE
fn paging_direct_map(
    mut pgt_allocator: impl FnMut() -> PhysicalAddr,
    max_addr: PhysicalAddr,
    cr3_addr: PhysicalAddr,
) {
    for addr in (0..max_addr).step_by(PAGE_SIZE) {
        arch_mm::init::add_page_mapping(
            &mut pgt_allocator,
            p2l_before_init,
            addr + arch_mm::P2L_OFFSET_BASE,
            addr,
            cr3_addr,
        );
    }
}

// Add kernel text mapping
pub fn paging_kernel_text_map(physical: PhysicalAddr, size: usize) {
    for addr in (physical..physical + size).step_by(PAGE_SIZE) {
        unsafe {
            arch_mm::init::add_page_mapping(
                &mut || allocate_page_table(&mut *(&raw mut PGT_MEMORY)),
                p2l_before_init,
                addr - physical + arch_mm::VMKERNEL_ENTRY_ADDRESS,
                addr,
                CR3_ADDR,
            )
        };
    }
}
