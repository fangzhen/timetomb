use core::arch::asm;

use timetomb::arch::x86_64::DescriptorTablePointer;

const GDT_ENTRY_COUNT: usize = 7;
pub static mut GDT_ENTRIES: [super::GdtEntry; GDT_ENTRY_COUNT] = [super::GdtEntry {
    limit15_0: 0,
    base15_0: 0,
    base23_16: 0,
    access_byte: 0x00,
    limit19_16_and_flags: 0x00,
    base31_24: 0,
}; GDT_ENTRY_COUNT];

pub static mut TSS_WITH_IO_MAP: super::TssWithIoMap = super::TssWithIoMap {
    tss: super::Tss {
        reserved0: 0,
        rsps: [0; 6],
        reserved1: 0,
        ists: [0; 14],
        reserved2: 0,
        reserved3: 0,
        iopb: 0,
        reserved4: 0,
        reserved5: 0,
    },
    io_permission_map: [0xff; 8192],
};

pub fn setup_gdt() {
    unsafe {
        let tss_base = (&raw const TSS_WITH_IO_MAP.tss) as *const super::Tss as u64;
        let io_map_base = (&raw const TSS_WITH_IO_MAP.io_permission_map) as *const _ as u64;
        TSS_WITH_IO_MAP.tss.iopb = (io_map_base - tss_base) as u16;
        GDT_ENTRIES = [
            //null
            super::GdtEntry {
                limit15_0: 0,
                base15_0: 0,
                base23_16: 0,
                access_byte: 0x00,
                limit19_16_and_flags: 0x00,
                base31_24: 0,
            },
            // kernel code
            super::GdtEntry {
                limit15_0: 0,
                base15_0: 0,
                base23_16: 0,
                access_byte: 0x9a, //1001 1010 non-conforming
                limit19_16_and_flags: 0xa0,
                base31_24: 0,
            },
            // kernel data /kernel ss
            super::GdtEntry {
                limit15_0: 0,
                base15_0: 0,
                base23_16: 0,
                access_byte: 0x92,
                limit19_16_and_flags: 0xa0,
                base31_24: 0,
            },
            // user data / user ss
            super::GdtEntry {
                limit15_0: 0,
                base15_0: 0,
                base23_16: 0,
                access_byte: 0xf2,
                limit19_16_and_flags: 0xa0,
                base31_24: 0,
            },
            // user code
            super::GdtEntry {
                limit15_0: 0,
                base15_0: 0,
                base23_16: 0,
                access_byte: 0xfa,
                limit19_16_and_flags: 0xa0,
                base31_24: 0,
            },
            // tss low
            super::GdtEntry {
                limit15_0: (core::mem::size_of::<super::TssWithIoMap>()) as u16,
                base15_0: (tss_base & 0xffff) as u16,
                base23_16: ((tss_base >> 16) & 0xff) as u8,
                access_byte: 0x89,
                limit19_16_and_flags: 0x00,
                base31_24: ((tss_base >> 24) & 0xff) as u8,
            },
            // tss high
            super::GdtEntry {
                limit15_0: ((tss_base >> 32) & 0xffff) as u16, // base47_32
                base15_0: ((tss_base >> 48) & 0xffff) as u16,  // base63_48
                base23_16: 0,
                access_byte: 0x00,
                limit19_16_and_flags: 0x00,
                base31_24: 0,
            },
        ];
        let gdt_size = core::mem::size_of::<super::GdtEntry>() * GDT_ENTRY_COUNT;
        let gdt_addr = DescriptorTablePointer {
            limit: gdt_size as u16,
            base: &GDT_ENTRIES[0] as *const super::GdtEntry as u64,
        };
        /*log::info!("GDT size: {:x}, addr: {:x}", gdt_size, {
            let b = gdt_addr.base;
            b
        });*/
        asm!(
            "lgdt [{gdt_addr}]",
            "mov ax, 0x28", // TSS low.  TODO(fangzhen) hardcode.
            "ltr ax",
            "push 0x08", // kernel code segment. TODO(fangzhen) hardcode.
            "lea rax, [2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            gdt_addr = in(reg) &gdt_addr,
            out("rax") _,
        );
        //log::info!("After lgdt");
    }
}
