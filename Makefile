#TODO(fangzhen) move to x86_64 related dirs
target_vm=target/x86_64-elf/debug
vm_bin=target/x86_64-elf/vmkernel.bin
target_boot=target/x86_64-uefi/debug

default: build

$(target_vm)/vmkernel: FORCE
	cargo build --target targets/x86_64-elf.json --bin vmkernel

$(vm_bin): $(target_vm)/vmkernel
	objcopy -S -O binary target/x86_64-elf/debug/vmkernel $(vm_bin)


$(target_boot)/boot.efi: $(vm_bin) FORCE
	md5sum $(vm_bin) | sed 's,^,// ,' > src/bin/boot/arch/x86_64/vmkernel.bin.hash
	cargo build --target x86_64-unknown-uefi --bin boot

build: $(target_boot)/boot.efi

run: build
	tools/runner target/x86_64-unknown-uefi/debug/boot.efi

clean:
	rm -rf target/

FORCE:
