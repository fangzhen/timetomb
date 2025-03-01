## 代码结构
main: vmkernel主体代码。目前会被链接为位置相关代码，使用虚拟内存。
boot：bootloader代码。目前有UEFI固件直接执行。进行初始化配置，并把控制流转移到vmkernel。
share: 两者共享的代码。两者内存模型不一致，因此share目录下的代码需要小心编写。尽量保持最小。
  目前只保留了memblock, uart和页表初始化相关的一些代码。
