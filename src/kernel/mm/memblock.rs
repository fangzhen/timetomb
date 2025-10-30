use core::cmp::min;

use log::info;
use timetomb::{
    arch::x86_64::mm::{MemoryDescriptor, MemoryType},
    kernel::mm::{PhysicalAddr, PAGE_FLAG_PHYSICAL, PAGE_SIZE},
};

use crate::arch::x86_64::mm::direct_map_p2l;

const MEMORY_REGION_COUNT: usize = 128;

#[derive(Copy, Clone)]
pub struct MemblockRegion {
    pub start: PhysicalAddr,
    pub size: usize,
    pub flags: u32,
}
pub struct MemblockType {
    pub cnt: usize,
    pub regions: [MemblockRegion; MEMORY_REGION_COUNT],
}

pub static mut ALL_MEMBLOCKS: MemblockType = MemblockType {
    cnt: 1,
    regions: [MemblockRegion {
        start: 0,
        size: 0,
        flags: 0,
    }; MEMORY_REGION_COUNT],
};
pub static mut USED_MEMBLOCKS: MemblockType = MemblockType {
    cnt: 1,
    regions: [MemblockRegion {
        start: 0,
        size: 0,
        flags: 0,
    }; MEMORY_REGION_COUNT],
};

pub fn generate_memblock_from_physical_map(uefi_map: &[MemoryDescriptor]) {
    for d in uefi_map {
        if d.mem_type != MemoryType::EfiReservedMemoryType {
            // TODO(fangzhen) EfiReservedMemoryType uses physical address with high address
            // in qemu
            let flag = PAGE_FLAG_PHYSICAL;
            add_memory(d.phys_start, d.page_count * PAGE_SIZE, flag);
            if d.mem_type != MemoryType::EfiConventionalMemory {
                // Simple set all memory except EfiConventionalMemory as used.
                // e.g. kernel itself is loaded by EFI firmware with type EfiLoaderCode.
                // UEFI system table resides in EfiRuntimeServicesData.
                add_used_memory(d.phys_start, d.page_count * PAGE_SIZE, flag);
            } else if d.phys_start == 0 {
                // TODO(fangzhen) mark address 0 as allocated to avoid allocate later.
                add_used_memory(0, PAGE_SIZE, flag);
            }
        }
    }
}

pub fn get_max_addr(mt: &MemblockType) -> PhysicalAddr {
    let mem_regions = mt.regions;
    let last_region = mem_regions[mt.cnt - 1];
    let max_physical = last_region.start + last_region.size;
    return max_physical;
}

//TODO
fn align_up(start: usize, align: usize) -> usize {
    return (start + align - 1) / align * align;
}
fn find_free_block(size: usize, align: usize) -> PhysicalAddr {
    let a_cnt = unsafe { ALL_MEMBLOCKS.cnt };
    let u_cnt = unsafe { USED_MEMBLOCKS.cnt };
    let a_regions = unsafe { &raw const ALL_MEMBLOCKS.regions };
    let u_regions = unsafe { &raw const USED_MEMBLOCKS.regions };
    let mut ia = 0;
    let mut iu = 0;
    let mut ur_start;
    let mut ur_end;
    while ia < a_cnt {
        let ar = unsafe { &(*a_regions)[ia] };
        let mut start_c = align_up(ar.start, align);
        while {
            if iu < u_cnt {
                let ur = unsafe { &(*u_regions)[iu] };
                ur_start = ur.start;
                ur_end = ur_start + ur.size;
            } else {
                ur_start = usize::MAX;
                ur_end = usize::MAX;
            }
            let end = min(ur_start, ar.start + ar.size);
            let free = end - start_c;
            if free >= size {
                return start_c;
            }

            start_c = align_up(ur_end, align);

            if ur_start < ar.start + ar.size {
                iu += 1;
            }
            ur_start < ar.start + ar.size
        } {}

        ia += 1;
    }
    return 0;
}

pub fn allocate_memory(start: usize, size: usize, align: usize, flag: u32) -> usize {
    return direct_map_p2l(allocate_physical_memory(start, size, align, flag));
}
pub fn allocate_physical_memory(
    mut start: usize,
    size: usize,
    align: usize,
    flag: u32,
) -> PhysicalAddr {
    if start == 0 {
        start = find_free_block(size, align);
    }
    add_range(
        unsafe { &mut *(&raw mut USED_MEMBLOCKS) },
        start,
        size,
        flag,
    );
    return start;
}

pub fn add_used_memory(start: usize, size: usize, flag: u32) {
    add_range(
        unsafe { &mut *(&raw mut USED_MEMBLOCKS) },
        start,
        size,
        flag,
    );
}
pub fn add_memory(start: usize, size: usize, flag: u32) {
    return add_range(unsafe { &mut *(&raw mut ALL_MEMBLOCKS) }, start, size, flag);
}
fn add_range(target: &mut MemblockType, start: usize, size: usize, flag: u32) {
    let end = start + size;
    let regions = &mut target.regions;
    for i in 0..target.cnt {
        let r = unsafe { &mut *(&mut regions[i] as *mut MemblockRegion) };
        let r_end = r.start + r.size;
        if end < r.start {
            //no overlap and insert
            if target.cnt >= regions.len() {
                info!("Too many memory regions.");
                panic!("Too many memory regions.");
            } else {
                for j in (i + 1..target.cnt).rev() {
                    regions[j] = regions[j - 1];
                }
                regions[i].start = start;
                regions[i].size = size;
                target.cnt += 1;
            }
            break;
        } else if end <= r_end {
            if start <= r.start {
                //overlap start - r.start - end - r_end; extend
                r.start = start;
                r.size = r_end - start;
            } else {
                // total coverd, nothing todo
            }
            break;
        } else {
            if start <= r_end {
                // overlap start/r.start - r_end - end; extend
                if start < r.start {
                    r.start = start;
                }
                r.size = end - r.start;
                // we may need to merge with next regions and move left.
                if i < target.cnt - 1 {
                    let mut merge_to = i;
                    for j in i + 1..target.cnt {
                        if end >= regions[j].start {
                            merge_to = j;
                        } else {
                            break;
                        }
                    }
                    if merge_to != i {
                        let delta = merge_to - i;
                        r.size = regions[merge_to].start + regions[merge_to].size - r.start;
                        for j in merge_to + 1..target.cnt {
                            regions[j - delta] = regions[j]
                        }
                        target.cnt -= delta;
                    }
                }
                break;
            } else {
                if i == target.cnt - 1 {
                    if target.cnt >= regions.len() {
                        info!("Too many memory regions.");
                        panic!("Too many memory regions.");
                    }
                    // no more to merge, add a new region.
                    regions[target.cnt] = MemblockRegion {
                        start,
                        size,
                        flags: flag,
                    };
                    target.cnt += 1;
                }
            }
        }
    }
}

pub fn print_memblocks() {
    unsafe {
        info!("All memory regions:");
        let regions = ALL_MEMBLOCKS.regions;

        for i in 0..ALL_MEMBLOCKS.cnt {
            let r = regions[i];
            info!(
                "#{}, start: {:#x},size: {:#x}",
                i,
                r.start,
                r.size,
                //regions[i].start + regions[i].size
            );
        }
        for i in 0..ALL_MEMBLOCKS.cnt {
            info!(
                "#{}, start: {:#x}, end: {:#x}, size: {:#x}",
                i,
                regions[i].start,
                regions[i].start + regions[i].size,
                regions[i].size
            );
        }

        info!("Used memory regions:");
        let regions = USED_MEMBLOCKS.regions;
        for i in 0..USED_MEMBLOCKS.cnt {
            info!(
                "#{}: start: {:#x}, end: {:#x}, size: {:#x}",
                i,
                regions[i].start,
                regions[i].start + regions[i].size,
                regions[i].size
            );
        }
    }
}
