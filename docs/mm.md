## 内存管理

1. 物理内存信息获取：使用UEFI `EFI_BOOT_SERVICES.GetMemoryMap`
2. 早期内存管理
3. 完整内存管理

### 早期内存管理
我们要让kernel来管理内存。而建立内存管理系统的过程本身也需要使用内存，所以需要一个过渡的内存管理系统。

1. 物理内存模型
   把固件的物理内存转换为kernel的物理内存模型。构建memblock内存管理。
   本身依赖：
     1. kernel text/data/stack可用，在当前实现中即沿用uefi下的配置。
     2. 不依赖内存分配，memblock数据结构本身需要的内存在kernel text/data中。
   提供功能： 以字节/页为单位进行分配回收

2. Enable 虚拟内存
   在memblock机制下准备好页表，并切换。
   切换需要保持当前代码运行的依赖，如gdt，kernel text, kernel stack等的地址保持不变。做到这一点，最简单的实现是让旧页表的映射在新页表还存在。具体来说，新页表包含identity map。

3. 内核栈/内核代码
   全部切换到新的虚拟地址空间。
   1. 包括gdt, kernel stack等。这些直接使用新的地址，旧的不再需要。
   2. kernel text/data: 需要让kernel代码中的指针符合新的虚拟地址。
      kernel代码要分成两部分：UEFI application，使用uefi的虚拟内存；vmkernel 使用kernel虚拟内存。需要从前者跳转至后者。
   3. setup 新的内存管理系统
     * 还依赖memblock。需要memblock本身的代码和数据结构在切换前后都可用。对于memblock代码，包含在两个二进制中；对于数据结构的地址，需要在上一步跳转时传过去。

5. 使用新的内存管理系统

内存使用：
 - boot text: 运行在uefi初始页表；不需要保留
 - boot data(包含memblock数据)：
   - gdt：不保留，不需要传数据？
   - memblock数据：不保留，需要传数据 - 考虑改为动态分配
 - boot期间memblock分配的内存
   - 页表：需保留，不需要传数据
   - kernel stack ?
   - kernel text & data: 需保留，不需要传数据
 - boot stack：不保留
 - kernel text & data：链接到固定地址；boot阶段不会执行，boot过程中被拷贝到kernel的链接地址；跳转完成从boot到kernel代码的切换。
 - uefi memory: 不保留，切换到vmkernel后不再使用uefi service.

### 物理内存模型
flat模型

### Virtual Memory Layout
我们的kernel需要构建自己的页表。我们目前只支持4级页表，页大小为4K。
| Start addr       | Offset     | End addr         | Size  | VM area description                                                                          |
|------------------|------------|------------------|-------|----------------------------------------------------------------------------------------------|
| ffff888000000000 | -119.5  TB | ffffc87fffffffff | 64 TB | direct mapping of all physical memory (page_offset_base)                                     |
| ffffffff80000000 |            |                  |       | kernel text                                                                                  |
| BY-UEFI-FIRMWARE |            |                  |       | kernel text and data loaded by uefi firmware. identity mapped. Deleted after vmkernel start. |

### 切换页表，切换内核栈
配置好页表后，更新`cr3`寄存器以切换到新的页表。
在切换页表前，先通过mmemblock分配器分配新的内核栈空间，在切换页表后立即切换到新内核栈。
切换页表和切换内核栈之间不要有任何栈操作，因为我们新的页表中没有映射原来的内核栈。

### image layout
```
+--------------+ uefi application
|              |
|--------------| __vmkernel_start
|              |
| vmkernel.bin |
|              |
|              |
|--------------| __vmkernel_end
|              |
+--------------+

```
vmkernel 通过linker script指定`.text.head` section 位于binary的最开始。
`head.rs`中`.text.head` section的代码直接跳转到`main`。
这样目的是使vmkernel的入口offset为0。
从而在uefi application跳转到vmkernel时，只需跳转到vmkernel的起始地址即可。

vmkernel 编译为位置相关的elf文件，否则需要处理relocation等。
uefi加载vmkernel.bin的位置需要和vmkernel 的编译时加载地址相同。

## 内存管理
### 内存管理形成过程

| 支撑     | 功能          |
| uefi     | memblock      |
| memblock | pages         |
| pages    | slab          |
| slab     | kmalloc       |
|          | vmalloc       |
|          | general alloc |

| 模块         | 条目                          | 分配     | 初始化                                  | 释放                                            |
| memblock     | ALL_MEMBLOCKS, USED_MEMBLOCKS | binary   | boot & vmkernel: generate from uefi map | page allocatorsetup之后就没用了，但没释放。leak |
| buddy system | MEM_ZONE                      | binary   |                                         | 不释放，一直存在                                |
| buddy system | MEM_ZONE.mem_map (Page frams) | memblock | physical.rs                             | 不释放，一直存在                                |

### 物理内存和页分配器
参考linux的物理内存管理，最简实现：
使用flat物理内存模型，使用page frame数组管理所有物理内存，所有物理内存归属于同一个zone（MEM_ZONE）。
采用buddy system的设计，分配和释放都以2^order大小的连续页面为单位。
不同于linux下的实现，没有使用bitmap来记录buddy的分配情况，而是把页面所属的order直接保存在了Page结构体中。
看到一个patch, linux kernel早期也有人尝试过不使用bitmap：<https://linux.kernel.narkive.com/eRSCX6sn/rfc-buddy-allocator-without-bitmap-2-0-3>。

### 虚拟内存
kmalloc
vmalloc
