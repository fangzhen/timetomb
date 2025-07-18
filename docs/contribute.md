## debug with qemu
```
make run run_param=gdb
```
Then use gdb to connect:
```
gdb -ex "target remote :9000" -ex "add-symbol-file target/x86_64-elf/debug/vmkernel"
```
