use timetomb::arch::x86_64::mm::init::add_page_mapping;
use timetomb::arch::x86_64::mm::init::memzero;
use timetomb::arch::x86_64::mm::p2l;
use timetomb::kernel::mm::{PAGE_SIZE, PhysicalAddr};

use crate::library::bitops;

use super::physical;

pub fn map_region(paddr: PhysicalAddr, size: usize, pml4_addr: PhysicalAddr) {
    let paddr_align = bitops::align_floor(paddr, PAGE_SIZE);
    for addr in (paddr_align..paddr + size).step_by(PAGE_SIZE) {
        add_page_mapping(
            &mut || {
                let paddr =
                    physical::MEM_ZONE.page_ref_to_paddr(physical::allocate_pages(1).unwrap());
                memzero(p2l(paddr), PAGE_SIZE);
                return paddr;
            },
            p2l,
            p2l(addr),
            addr,
            pml4_addr,
        );
    }
}
