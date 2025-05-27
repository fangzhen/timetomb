use core::ptr::NonNull;

use acpi::{AcpiHandler, PhysicalMapping};
use timetomb::{arch::x86_64::mm::p2l, kernel::mm::PhysicalAddr};

use crate::{arch::x86_64::mm::CR3_ADDR, kernel::mm::paging};

#[derive(Clone, Copy)]
pub struct MemblockAcpiHandler {}

impl AcpiHandler for MemblockAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: PhysicalAddr,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        log::info!(
            "Map physical region: addr: {:#x}, size: {:#x}",
            physical_address,
            size
        );
        let va = p2l(physical_address);
        unsafe { paging::map_region(physical_address, size, CR3_ADDR) };
        let object = unsafe { NonNull::new_unchecked(va as *mut T) };
        return unsafe { PhysicalMapping::new(physical_address, object, size, size, *self) };
    }

    fn unmap_physical_region<T>(_region: &acpi::PhysicalMapping<Self, T>) {
        //TODO(fangzhen)
    }
}
