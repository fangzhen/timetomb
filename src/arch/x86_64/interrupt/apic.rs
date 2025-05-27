use core::arch::asm;

use crate::arch::x86_64::instruction_wrappers::{cpuid, inb, outb, rdmsr};
use crate::arch::x86_64::mm::CR3_ADDR;
use crate::driver::acpi as acpi_driver;
use crate::kernel::mm::paging;
use acpi::madt;
use timetomb::arch::x86_64::mm::p2l;

const CPUID_APIC_FLAG: u32 = 1 << 9;
const IA32_APIC_BASE_MSR: u32 = 0x1b;
const IA32_APIC_BASE_MSR_ENABLE: u64 = 1 << 11;

const LAPIC_OFFSET_EOI: usize = 0xb0;
const LAPIC_OFFSET_TPR: usize = 0x80;
const LAPIC_OFFSET_SVR: usize = 0xf0;
const LAPIC_OFFSET_TIMER: usize = 0x320;
const LAPIC_OFFSET_LINT1: usize = 0x360;
const LAPIC_OFFSET_TMRINITCNT: usize = 0x380;
const LAPIC_OFFSET_TMRCURRCNT: usize = 0x390;
const LAPIC_OFFSET_TMRDIV: usize = 0x3e0;

const APIC_SW_ENABLE: u32 = 0x100;

// PIC
const MASTER_PIC: u16 = 0x20;
const SLAVE_PIC: u16 = 0xa0;
const MASTER_PIC_OFFSET: u8 = 0x20;
const SLAVE_PIC_OFFSET: u8 = 0x28;

const IOAPIC_IRQ_OFFSET: u32 = 0x30;
static mut LAPIC_BASE: usize = 0;

fn disable_pic() {
    // 1. init PIC and remap irqs to avoid conflicts with exceptions and
    //    in case of spurious interrupts.
    outb(MASTER_PIC, 0x11); //ICW1
    outb(SLAVE_PIC, 0x11);
    outb(MASTER_PIC + 1, MASTER_PIC_OFFSET); //ICW2
    outb(SLAVE_PIC + 1, SLAVE_PIC_OFFSET);
    outb(MASTER_PIC + 1, 4); //ICW3
    outb(SLAVE_PIC + 1, 2);
    outb(MASTER_PIC + 1, 1); //ICW4
    outb(SLAVE_PIC + 1, 1);

    // 2. mask all PIC interrupts
    outb(MASTER_PIC + 1, 0xff); //OCW1
    outb(SLAVE_PIC + 1, 0xff); //OCW1
}

#[inline(always)]
pub fn read_lapic_register(base: usize, offset: usize) -> u32 {
    unsafe {
        return *((base + offset) as *mut u32);
    }
}

#[inline(always)]
pub fn write_lapic_register(base: usize, offset: usize, value: u32) {
    unsafe {
        *((base + offset) as *mut u32) = value;
    }
}

#[inline(always)]
pub fn read_ioapic_register(base: usize, offset: u32) -> u32 {
    unsafe {
        *((base) as *mut u32) = offset;
        return *((base + 0x10) as *mut u32);
    }
}

#[inline(always)]
pub fn write_ioapic_register(base: usize, offset: u32, value: u32) {
    unsafe {
        *((base) as *mut u32) = offset;
        *((base + 0x10) as *mut u32) = value;
    }
}

fn init_lapic_timer(lapic_base: usize) {
    // init lapic timer to ont-shot mode
    write_lapic_register(lapic_base, LAPIC_OFFSET_TIMER, 0x30);
    // divide by 16
    write_lapic_register(lapic_base, LAPIC_OFFSET_TMRDIV, 0x3);
    unsafe {
        asm!(
            "mov dx, 61h",
            "mov al, 10110010b", //PIT mode 1: one-shot on channel 2, Access mode: lobyte/hibyte
            "out 43h, al",
            //1193180/100 Hz = 11931 = 2e9bh
            "mov al, 9bh", //LSB
            "out 42h, al",
            "mov al, 2eh", //MSB
            "out 42h, al",
            //reset PIT one-shot counter (start counting)
            "in al, dx",
            "and al, 0feh",
            "out dx, al", //gate low
            "or al, 1 ",
            "out dx, al", //gate high. Rising edge of the gate starts pit to counting down on the next falling edge of the (1.193182 MHz) input signal

            out("al") _,
            out("dx") _,
        );
    }
    //reset APIC timer (set counter to u32::MAX)
    write_lapic_register(lapic_base, LAPIC_OFFSET_TMRINITCNT, u32::MAX);
    //now wait until PIT counter reaches zero (output goes high)
    while inb(0x61) & 0x20 == 0 {}
    let currcnt = read_lapic_register(lapic_base, LAPIC_OFFSET_TMRCURRCNT);

    //start lacpi timer:  every 10 ms, with vector 0x30.
    let ms_10 = u32::MAX - currcnt;
    write_lapic_register(lapic_base, LAPIC_OFFSET_TMRINITCNT, ms_10 * 10); // timer: every 100ms
    write_lapic_register(lapic_base, LAPIC_OFFSET_TIMER, 0x20030);
}

fn init_lapic(rsdp_addr: usize) {
    let acpi_handler = acpi_driver::MemblockAcpiHandler {};
    let mut lapic_addr: usize = 0;
    let mut ioapic_addr: usize = 0;

    #[derive(Copy, Clone)]
    struct ISOEntry {
        irq: u8,
        gsi: u32,
    }
    struct InterruptSourceOverrideEntries {
        size: u8,
        entries: [ISOEntry; 16],
    }
    let mut ises = InterruptSourceOverrideEntries {
        size: 0,
        entries: [ISOEntry { irq: 0, gsi: 0 }; 16],
    };

    unsafe {
        match acpi::AcpiTables::from_rsdp(acpi_handler, rsdp_addr) {
            Ok(acpi_tables) => {
                if let Ok(madt_table) = acpi_tables.find_table::<madt::Madt>() {
                    lapic_addr = madt_table.local_apic_address as usize;
                    for entry in madt_table.entries() {
                        match entry {
                            madt::MadtEntry::LocalApicAddressOverride(addr_override) => {
                                lapic_addr = addr_override.local_apic_address as usize;
                            }
                            madt::MadtEntry::IoApic(ioapic) => {
                                ioapic_addr = ioapic.io_apic_address as usize;
                                log::info!("IOAPIC Address: {:#x}", ioapic_addr);
                                log::info!("IOAPIC GSI base: {:#x}", {
                                    let b = ioapic.global_system_interrupt_base;
                                    b
                                });
                            }
                            madt::MadtEntry::LocalApic(lapic) => {
                                let lid = lapic.processor_id;
                                log::info!("LAPIC ID: {:#x}", lid);
                            }
                            madt::MadtEntry::InterruptSourceOverride(ov) => {
                                ises.entries[ises.size as usize].irq = ov.irq;
                                ises.entries[ises.size as usize].gsi = ov.global_system_interrupt;
                                ises.size += 1;
                            }
                            _ => log::info!("Other"),
                        }
                    }
                }
            }
            Err(e) => log::info!("Failed to construct ACPI Tables {:?}", e),
        }
    }

    // setup LVT:
    log::info!("Local APIC Address: {:#x}", lapic_addr);
    //TODO: mmio memory space should be strong uncachable.
    paging::map_region(lapic_addr as usize, 0x3f0, unsafe { CR3_ADDR });
    lapic_addr = p2l(lapic_addr);
    unsafe { LAPIC_BASE = lapic_addr };

    // We masked all 8259 PIC IRQs, as a result, LINT0 is not used.
    // LINT1 : The local APIC’s NMI source.
    // NMI relation with lintn is also reported by ACPI.
    // Just hardcode for now.
    let lint1 = 0x400; // Delivery mode: NMI
    write_lapic_register(lapic_addr, LAPIC_OFFSET_LINT1, lint1);

    let svr = 0xff; // spurious vertor number: 0xff
    write_lapic_register(lapic_addr, LAPIC_OFFSET_SVR, svr | APIC_SW_ENABLE);

    // Allow interrupts of all priorities. 0 is default.
    write_lapic_register(lapic_addr, LAPIC_OFFSET_TPR, 0);

    init_lapic_timer(lapic_addr);

    //TODO ioapic related code temp here.
    //uart - irq4 - com1
    let mut uart_gsi = 4;
    for i in 0..ises.size {
        if ises.entries[i as usize].irq as u32 == uart_gsi {
            uart_gsi = ises.entries[i as usize].gsi;
            break;
        }
    }

    //TODO size
    paging::map_region(ioapic_addr as usize, 0x20, unsafe { CR3_ADDR });
    ioapic_addr = p2l(ioapic_addr);

    // TODO: check gsi of uart in range of this ioapic
    let uart_tbl = (IOAPIC_IRQ_OFFSET + uart_gsi) as u64;
    write_ioapic_register(ioapic_addr, 0x10 + uart_gsi * 2, uart_tbl as u32);
    write_ioapic_register(
        ioapic_addr,
        0x10 + uart_gsi * 2 + 1,
        (uart_tbl >> 32) as u32,
    );
}

fn enable_local_apic(rsdp_addr: usize) {
    log::info!("Trying to setup local APIC");
    // 1. xapic mode is default to enabled after init. Check that.
    let apic_base_msr = rdmsr(IA32_APIC_BASE_MSR);
    log::info!("APIC {:#x}", apic_base_msr);
    if apic_base_msr & IA32_APIC_BASE_MSR_ENABLE == 0 {
        panic!("xapic is disabled!")
    }

    // 2. Disable 8259 PIC:
    disable_pic();

    // 3. Init apic
    init_lapic(rsdp_addr);
}

pub fn init_apic(rsdp_addr: usize) {
    let cpuid = cpuid(0x01);
    if cpuid.edx & CPUID_APIC_FLAG == 0 {
        panic!("No builtin local APIC!")
    }
    enable_local_apic(rsdp_addr);
}

pub fn send_eoi() {
    unsafe { write_lapic_register(LAPIC_BASE, LAPIC_OFFSET_EOI, 0) }
}
