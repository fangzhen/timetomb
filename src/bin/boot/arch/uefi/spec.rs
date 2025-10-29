use core::ffi::c_void;
pub use timetomb::arch::x86_64::mm::{MemoryDescriptor, MemoryType};
pub type Status = usize;
pub const EFI_ACPI_20_TABLE_GUID: Guid = Guid {
    d1: 0x8868e871,
    d2: 0xe4f1,
    d3: 0x11d3,
    d4: [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
};

/// EFI_TABLE_HEADER
#[derive(Debug)]
#[repr(C)]
pub struct Header {
    pub signature: u64,
    pub revision: u32,
    pub size: u32,
    pub crc: u32,
    _reserved: u32,
}

pub type Handle = *mut c_void;
type Ignore = usize;
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Guid {
    pub d1: u32,
    pub d2: u16,
    pub d3: u16,
    pub d4: [u8; 8],
}

#[repr(C)]
pub struct Output {
    pub reset: unsafe extern "efiapi" fn(this: &Output, extended: bool) -> Status,
    pub output_string: unsafe extern "efiapi" fn(this: &Output, string: *const u16) -> Status,
    pub test_string: unsafe extern "efiapi" fn(this: &Output, string: *const u16) -> Status,
    pub query_mode: unsafe extern "efiapi" fn(
        this: &Output,
        mode: usize,
        columns: &mut usize,
        rows: &mut usize,
    ) -> Status,
    pub set_mode: unsafe extern "efiapi" fn(this: &mut Output, mode: usize) -> Status,
    pub set_attribute: unsafe extern "efiapi" fn(this: &mut Output, attribute: usize) -> Status,
    pub clear_screen: unsafe extern "efiapi" fn(this: &mut Output) -> Status,
    pub set_cursor_position:
        unsafe extern "efiapi" fn(this: &mut Output, column: usize, row: usize) -> Status,
    pub enable_cursor: unsafe extern "efiapi" fn(this: &mut Output, visible: bool) -> Status,
    pub data: Ignore,
}

#[allow(dead_code)]
#[derive(PartialEq, Debug, Copy, Clone)]
#[repr(C)]
pub enum AllocateType {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
    MaxAllocateType,
}
pub type MemoryMapKey = usize;

#[repr(C)]
pub struct BootServices {
    pub header: Header,

    // Task Priority services
    pub raise_tpl: Ignore,
    pub restore_tpl: Ignore,

    // Memory allocation functions
    pub allocate_pages: unsafe extern "efiapi" fn(
        alloc_ty: AllocateType,
        mem_ty: MemoryType,
        count: usize,
        addr: *mut u8,
    ) -> Status,
    pub free_pages: unsafe extern "efiapi" fn(addr: u64, pages: usize) -> Status,
    pub get_memory_map: unsafe extern "efiapi" fn(
        size: &mut usize,
        map: *mut MemoryDescriptor,
        key: &mut MemoryMapKey,
        desc_size: &mut usize,
        desc_version: &mut u32,
    ) -> Status,
    pub allocate_pool: unsafe extern "efiapi" fn(
        pool_type: MemoryType,
        size: usize,
        buffer: *mut *mut u8,
    ) -> Status,
    pub free_pool: unsafe extern "efiapi" fn(buffer: *mut u8) -> Status,

    // Event & timer functions
    pub create_event: Ignore,
    pub set_timer: Ignore,
    pub wait_for_event: Ignore,
    pub signal_event: Ignore,
    pub close_event: Ignore,
    pub check_event: Ignore,

    // Protocol handlers
    pub install_protocol_interface: Ignore,
    pub reinstall_protocol_interface: Ignore,
    pub uninstall_protocol_interface: Ignore,
    pub handle_protocol: unsafe extern "efiapi" fn(
        handle: Handle,
        proto: &Guid,
        out_proto: &mut *mut c_void,
    ) -> Status,
    pub _reserved: Ignore,
    pub register_protocol_notify: Ignore,
    pub locate_handle: unsafe extern "efiapi" fn(
        search_ty: i32,
        proto: *const Guid,
        key: *mut c_void,
        buf_sz: &mut usize,
        buf: *mut Handle,
    ) -> Status,
    pub locate_device_path: Ignore,
    pub install_configuration_table: Ignore,

    // Image services
    pub load_image: Ignore,
    pub start_image: Ignore,
    pub exit: Ignore,
    pub unload_image: Ignore,
    pub exit_boot_services:
        unsafe extern "efiapi" fn(image_handle: Handle, map_key: MemoryMapKey) -> Status,

    // Misc services
    pub get_next_monotonic_count: Ignore,
    pub stall: unsafe extern "efiapi" fn(microseconds: usize) -> Status,
    pub set_watchdog_timer: Ignore,

    // Driver support services
    pub connect_controller: Ignore,
    pub disconnect_controller: Ignore,

    // Protocol open / close services
    pub open_protocol: Ignore,
    pub close_protocol: Ignore,
    pub open_protocol_information: Ignore,

    // Library services
    pub protocols_per_handle: Ignore,
    pub locate_handle_buffer: Ignore,
    pub locate_protocol: unsafe extern "efiapi" fn(
        proto: &Guid,
        registration: *mut c_void,
        out_proto: &mut *mut c_void,
    ) -> Status,
    pub install_multiple_protocol_interfaces: Ignore,
    pub uninstall_multiple_protocol_interfaces: Ignore,

    // CRC services
    pub calculate_crc32: Ignore,

    // Misc services
    pub copy_mem: unsafe extern "efiapi" fn(dest: *mut u8, src: *const u8, len: usize),
    pub set_mem: unsafe extern "efiapi" fn(buffer: *mut u8, len: usize, value: u8),

    // New event functions (UEFI 2.0 or newer)
    pub create_event_ex: Ignore,
}

#[repr(C)]
pub struct ConfigurationTable {
    pub vendor_guid: Guid,
    pub vendor_table: *const c_void,
}

/// EFI_SYSTEM_TABLE
#[derive(Debug)]
#[repr(C)]
pub struct SystemTable {
    pub header: Header,
    pub fw_vendor: *const u16,
    pub fw_revision: u32,
    pub stdin_handle: Handle,
    pub stdin: Ignore,
    pub stdout_handle: Handle,
    pub stdout: *mut Output,
    pub stderr_handle: Handle,
    pub stderr: Ignore,
    pub runtime: Ignore,
    pub boot: *const BootServices,
    pub nr_cfg: usize,
    pub cfg_table: *const ConfigurationTable,
}
