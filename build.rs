use std::env;
use std::format;
use std::fs;
use std::path::Path;

const VMKERNEL_ENTRY_ADDRESS: usize = 0xffffffff80000000;
const SETUP_HEADER_OFFSET: usize = 16;
const SETUP_HEADER_SIZE: usize = 1024;
//TODO check SETUP_HEADER_OFFSET/ SETUP_HEADER_SIZE

fn main() {
    // Ensure vmlinux.lds is consistent with boot code.
    let out_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let src_path = Path::new(&out_dir).join("vmkernel.lds.tpl");
    let dest_path = Path::new(&out_dir).join("vmkernel.lds");
    let tpl = fs::read_to_string(&src_path).unwrap();
    fs::write(
        &dest_path,
        tpl.replace(
            "VMKERNEL_ENTRY_ADDRESS_REPLACE",
            &(format!("0x{:x}", VMKERNEL_ENTRY_ADDRESS)),
        ),
    )
    .unwrap();

    let src_path = Path::new(&out_dir).join("src/head.rs.tpl");
    let dest_path = Path::new(&out_dir).join("src/head.rs");
    let tpl = fs::read_to_string(&src_path).unwrap();
    fs::write(
        &dest_path,
        tpl.replace("SETUP_HEADER_SIZE", &(format!("0x{:x}", SETUP_HEADER_SIZE)))
            .replace(
                "SETUP_HEADER_OFFSET",
                &(format!("0x{:x}", SETUP_HEADER_OFFSET)),
            ),
    )
    .unwrap();

    let ffi_shared_path = Path::new(&out_dir).join("src/share/arch/x86_64/ffi_shared.rs");
    println!("{}", ffi_shared_path.display());
    fs::write(
        &&ffi_shared_path,
        vec![
            format!(
                "pub const VMKERNEL_ENTRY_ADDRESS: usize = 0x{:x};",
                VMKERNEL_ENTRY_ADDRESS
            ),
            format!(
                "pub const SETUP_HEADER_SIZE: usize = 0x{:x};",
                SETUP_HEADER_SIZE
            ),
            format!(
                "pub const SETUP_HEADER_OFFSET: usize = 0x{:x};",
                SETUP_HEADER_OFFSET
            ),
        ]
        .join("\n")
            + "\n",
    )
    .unwrap();

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=vmkernel.lds.tpl");
}
