pub mod spec;
pub mod writer;

use core::ptr;
use log::info;
use spec::*;

pub static mut UEFI_SYSTEM_TAB: *const spec::SystemTable = core::ptr::null_mut();

impl SystemTable {
    pub fn clear_screen(self: &SystemTable) {
        let o = unsafe { self.stdout.as_mut() };
        if let Some(x) = o {
            unsafe {
                (x.clear_screen)(x);
            }
        }
    }
}

pub fn allocate_pool<T>(
    st: &SystemTable,
    mtype: MemoryType,
    size: usize,
    addr: &mut *mut T,
) -> Status {
    let bs = unsafe { st.boot.as_ref().unwrap() };
    unsafe {
        return (bs.allocate_pool)(mtype, size, addr as *mut *mut T as *mut *mut u8);
    }
}

pub fn get_memory_map(st: &SystemTable) -> (MemoryMapKey, &[MemoryDescriptor]) {
    let mut size: usize = 0;
    let mut map: *mut MemoryDescriptor = ptr::null_mut();
    let mut key: MemoryMapKey = 0;
    let mut desc_size: usize = 0;
    let mut desc_version: u32 = 0;
    let mut descriptors_raw: *mut MemoryDescriptor = ptr::null_mut();
    let descriptors;
    let desc_count;
    info!("Retrieving UEFI memory map:");
    let bs = unsafe { st.boot.as_ref().unwrap() };
    loop {
        unsafe {
            // get_memory_map: 1st call to get size of memory_map; 2nd call to
            // get whole memory map.
            (bs.get_memory_map)(&mut size, map, &mut key, &mut desc_size, &mut desc_version);
            size += desc_size * 2;
            allocate_pool(st, MemoryType::EfiLoaderData, size, &mut map);
            allocate_pool(st, MemoryType::EfiLoaderData, size, &mut descriptors_raw);

            if (bs.get_memory_map)(&mut size, map, &mut key, &mut desc_size, &mut desc_version) == 0
            {
                desc_count = size / desc_size;
                // Construct descriptors from map. We cannot get descriptors directly from
                // from_raw_parts(map, desc_count), since size_of MemoryDescriptor <= desc_size.
                descriptors = core::slice::from_raw_parts_mut(descriptors_raw, desc_count);
                for i in 0..desc_count {
                    let d = ((map as usize) + desc_size * i) as *const MemoryDescriptor;
                    descriptors[i] = *d
                }
                break;
            }
        }
    }
    return (key, descriptors);
}

pub fn exit_boot_services(hdr: Handle, st: &SystemTable) -> &[MemoryDescriptor] {
    info!("UEFI exit_boot_service");
    let bs = unsafe { st.boot.as_ref().unwrap() };
    loop {
        let (key, map) = get_memory_map(st);
        let ret = unsafe { (bs.exit_boot_services)(hdr, key) };
        if ret == 0 {
            return map;
        }
    }
}

pub fn get_rsdp_addr() -> usize {
    let st = unsafe { UEFI_SYSTEM_TAB.as_ref().unwrap() };

    let ctn = st.nr_cfg;
    let ct = unsafe { core::slice::from_raw_parts(st.cfg_table, ctn) };
    for c in ct {
        if c.vendor_guid == spec::EFI_ACPI_20_TABLE_GUID {
            info!("Found acpi 2.0 table at {:#x}.", c.vendor_table as usize);
            return c.vendor_table as usize;
        }
    }
    panic!("Cannot find ACPI table in UEFI Configuration Table.");
}
