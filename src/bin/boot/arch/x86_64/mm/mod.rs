use timetomb::arch::x86_64::mm as arch_mm;
use timetomb::kernel::mm::{LinearAddr, PhysicalAddr, PAGE_SIZE};

#[derive(Debug)]
pub struct PgtMemory {
    pub current: usize,
}

fn p2l_before_init(physical: PhysicalAddr) -> LinearAddr {
    return physical;
}
core::arch::global_asm!(
    //
    ".align 4096", // TODO pagesize
    ".section .boot_data, \"aw\"",
    ".globl _boot_pgtable",
    "_boot_pgtable:",
    ".fill 40960, 1, 0"
);
unsafe extern "C" {
    static _boot_pgtable: u8;
}

pub static mut PGT_MEMORY: PgtMemory = PgtMemory { current: 0 };

// allocate physical page for page tables.
fn allocate_page_table(pages: &mut PgtMemory) -> PhysicalAddr {
    let physical_addr = pages.current;
    pages.current += PAGE_SIZE;
    arch_mm::init::memzero(p2l_before_init(physical_addr), PAGE_SIZE);
    return physical_addr;
}

// Add kernel text mapping
pub fn paging_kernel_text_map(physical: PhysicalAddr, size: usize) {
    let cr3: usize;
    unsafe {
        PGT_MEMORY.current = &_boot_pgtable as *const u8 as usize;

        core::arch::asm!(
            "mov {}, cr3",
            "mov rax, cr0",
            "btr eax, 16", // Reset WP bit to allow write to uefi page table
            "mov cr0, rax",
            out(reg) cr3,
            out("rax") _,
        );
    }
    for addr in (physical..physical + size).step_by(PAGE_SIZE) {
        unsafe {
            arch_mm::init::add_page_mapping(
                &mut || allocate_page_table(&mut *(&raw mut PGT_MEMORY)),
                p2l_before_init,
                addr + arch_mm::VMKERNEL_ENTRY_ADDRESS - physical,
                addr,
                cr3,
            )
        };
    }
}
