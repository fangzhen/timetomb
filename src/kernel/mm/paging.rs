use timetomb::arch::x86_64::ffi_shared::VMKERNEL_ENTRY_ADDRESS;
use timetomb::arch::x86_64::mm::MemoryDescriptor;
use timetomb::arch::x86_64::mm::MemoryType;
// TODO x86_64
use timetomb::arch::x86_64::mm as arch_mm;
use timetomb::arch::x86_64::mm::add_page_mapping;
use timetomb::arch::x86_64::mm::memzero;
use timetomb::arch::x86_64::SetupHeader;
use timetomb::kernel::mm::{LinearAddr, PhysicalAddr, PAGE_SIZE};

use crate::arch::x86_64::mm::direct_map_p2l;
use crate::library::bitops;
static mut VMKERNEL_TEXT_OFFSET: usize = 0;
pub static mut INIT_PT_ADDR: usize = 0;

use super::physical;
unsafe extern "C" {
    static _pgtable_start: u8;
}

/// Add a region of physical addr to direct map
pub fn map_region(paddr: PhysicalAddr, size: usize) {
    let pml4_addr: PhysicalAddr;
    unsafe {
        core::arch::asm!( "mov {}, cr3", out(reg) pml4_addr);
    }
    let pml4_addr = pml4_addr & 0x000ffffffffff000;
    let paddr_align = bitops::align_floor(paddr, PAGE_SIZE);
    for addr in (paddr_align..paddr + size).step_by(PAGE_SIZE) {
        add_page_mapping(
            &mut || {
                let paddr =
                    physical::MEM_ZONE.page_ref_to_paddr(physical::allocate_pages(1).unwrap());
                memzero(direct_map_p2l(paddr), PAGE_SIZE);
                return paddr;
            },
            direct_map_p2l,
            direct_map_p2l(addr),
            addr,
            pml4_addr,
        );
    }
}

#[derive(Debug)]
pub struct PgtMemory {
    pub max: usize,
    pub current: usize,
}

pub static mut PGT_MEMORY: PgtMemory = PgtMemory { max: 0, current: 0 };
pub fn kernel_text_p2l(physical: PhysicalAddr) -> LinearAddr {
    return unsafe { physical + VMKERNEL_TEXT_OFFSET };
}

// allocate physical page for page tables.
fn allocate_page_table(pages: &mut PgtMemory) -> PhysicalAddr {
    let linear_addr = pages.current;
    pages.current += PAGE_SIZE;
    arch_mm::memzero(linear_addr, PAGE_SIZE);
    let physical_addr = unsafe { linear_addr - VMKERNEL_TEXT_OFFSET };
    return physical_addr;
}

// calulate max pages needed for page tabel to map linear address 0..max_addr
pub fn max_pt_pages(max_addr: usize) -> usize {
    return 4 + (max_addr >> 39) + (max_addr >> 30) + (max_addr >> 21);
}

pub fn init_paging(setup_header: &SetupHeader) -> PhysicalAddr {
    unsafe { VMKERNEL_TEXT_OFFSET = VMKERNEL_ENTRY_ADDRESS - setup_header.kernel_physical };
    let uefi_map = unsafe {
        core::slice::from_raw_parts(
            setup_header.mem_desc as *const _,
            setup_header.mem_desc_count,
        )
    };
    let cr3_addr = init_page_mapping(uefi_map);
    paging_kernel_text_map(
        setup_header.kernel_physical,
        setup_header.kernel_size,
        cr3_addr,
    );
    unsafe {
        core::arch::asm!(
            "mov rax, 0x000ffffffffff000",
            "and rdi, rax", // clear reserved bits.
            "mov cr3, rdi",

            //Force flush TLB
            "mov rcx, cr4",
            "mov rax, rcx",
            "xor rcx, 128", // PGE
            "mov cr4, rcx",
            "mov cr4, rax",

            in("rdi") cr3_addr,
            out("rax") _,
            out("rcx") _,
        )
    }

    log::info!("We are using new page table now!");

    unsafe { INIT_PT_ADDR = cr3_addr };
    return cr3_addr;
}

pub fn init_page_mapping(uefi_map: &[MemoryDescriptor]) -> PhysicalAddr {
    let mut last = uefi_map[0];
    for item in uefi_map.iter().rev() {
        if item.mem_type != MemoryType::EfiReservedMemoryType {
            last = *item;
            break;
        }
    }
    let max_physical = last.phys_start + last.page_count * PAGE_SIZE;
    let pgt_addr = unsafe { &_pgtable_start as *const u8 as usize };
    unsafe {
        PGT_MEMORY.current = pgt_addr;
        let cr3_addr = allocate_page_table(&mut *(&raw mut PGT_MEMORY));
        paging_direct_map(
            || allocate_page_table(&mut *(&raw mut PGT_MEMORY)),
            max_physical,
            cr3_addr,
        );
        return cr3_addr;
    }
}

// Mapping whole physical memory to virtual address with offset P2L_OFFSET_BASE
fn paging_direct_map(
    mut pgt_allocator: impl FnMut() -> PhysicalAddr,
    max_addr: PhysicalAddr,
    cr3_addr: PhysicalAddr,
) {
    for addr in (0..max_addr).step_by(PAGE_SIZE) {
        arch_mm::add_page_mapping(
            &mut pgt_allocator,
            kernel_text_p2l,
            direct_map_p2l(addr),
            addr,
            cr3_addr,
        );
    }
}

// Add kernel text mapping
pub fn paging_kernel_text_map(physical: PhysicalAddr, size: usize, cr3_addr: PhysicalAddr) {
    for addr in (physical..physical + size).step_by(PAGE_SIZE) {
        unsafe {
            arch_mm::add_page_mapping(
                &mut || allocate_page_table(&mut *(&raw mut PGT_MEMORY)),
                kernel_text_p2l,
                addr + arch_mm::VMKERNEL_ENTRY_ADDRESS - physical,
                addr,
                cr3_addr,
            )
        };
    }
}
