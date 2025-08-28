## 内存管理
以下都是指x86_64架构下。

1. UEFI提供的内存管理
2. 早期内存管理
3. 完整内存管理

### UEFI运行时
UEFI固件为其上运行的UEFI程序准备的运行时环境：
- CPU 模式：x86_64 long mode, CPL=0
- 分页：4级页表，采用identity 映射。
- GDT：包含一个简单的，扁平模型的GDT。长模式下已无分段。
- IDT：包含一个IDT，用于调试和必要的硬件处理。
- 栈：UEFI固件加载UEFI应用时，会为其分配栈。

Boot service中的内存相关API：

- `GetMemoryMap`：获取系统内存分布情况
- `AllocatePool/FreePool`：按页分配/回收内存
- `AllocatePages/FreePages`：按字节分配/回收内存


vmkernel需求：
1. 加载地址在`VMKERNEL_ENTRY_ADDRESS`，运行时至还需要以下内存：
   - 内核栈 (kernel text)
   - 内存段、页初始化
     - 新page table本身，包含direct map、kernel text (kernel text)
     - 新GDT、TSS、IDT
   - memblock系统建立
     - 硬件MemoryMap（uefi map）
     - memblock所需内存
   - 物理内存管理 - struct page array
   - slab
     - 动态分配（memblock）

先setup页表，再建立memblock：
  - memblock的逻辑不需要处理页表切换前后不同的虚拟/物理地址映射。
  - 但是，页表本身也无法用memblock动态分配内存。而且uefi map需要在页表切换后可以访问。（目前的实现是uefi map复制到kernel text后面）

### 内存管理系统建立过程

| uefi app load      | exit boot service | memblock setup | switch to new page table | goto kernel        | kernel mm init | rebuild memblock | kernel mm | setup idt |
|--------------------|-------------------|----------------|--------------------------|--------------------|----------------|------------------|-----------|-----------|
| uefi runtime       | ❎                |                |                          |                    |                |                  |           |           |
|                    | 获取uefi map      |                |                          |                    |                | ❎               |           |           |
| uefi identity map  |                   |                | ❎                       |                    |                |                  |           |           |
| uefi app stack     |                   |                | ❎                       |                    |                |                  |           |           |
|                    |                   |                | kernel identity map      |                    |                | ❎               |           |           |
|                    |                   |                | kernel direct map        |                    |                |                  |           |           |
|                    |                   |                | kernel stack             |                    |                |                  |           |           |
| uefi app text&data |                   |                |                          | ❎                 |                |                  |           |           |
|                    |                   |                |                          | vmkernel text&data |                |                  |           |           |
| uefi gdt           |                   |                |                          |                    | ❎             |                  |           |           |
|                    |                   |                |                          |                    | new gdt        |                  |           |           |
| uefi idt           |                   |                |                          |                    |                |                  |           | ❎        |
|                    |                   |                |                          |                    |                |                  |           | new idt   |

每列表示启动的不同阶段中，分配或释放的内存区域。❎ 表示对应项不再需要，对应内存可以释放

bootloader阶段：
- uefi app load: uefi runtime加载uefi格式的内核镜像，并分配相关内存。
- exit boot service: 获取uefi map并退出uefi boot service；
- memblock setup：不依赖动态分配内存，memblock本身需要的内存静态编译在uefi app的静态变量；使用uefi map的信息；
- switch to new page table: 依赖memblock分配页表页；setup direct map + identity map; 分配新kernel stack；切换到新页表和stack。
- go to kernel: 把vmkernel从uefi加载的位置复制到新分配的地址，并跳转。vmkernel data section 包含一个setup header区域，用于从bootloader向kernel传数据。

vmkernel阶段：vmkernel需要放在`VMKERNEL_ENTRY_ADDRESS` 虚拟地址处，因为链接时链接到了该地址。
- kernel mm init: setup gdt. GDT本身的内存在kernel data section, 不需要动态分配
- rebuild memblock：从uefi map重新setup memblock。 这里uefi map数据结构本身的地址还是物理地址，因此还需要identity map。把内存中还有用的部分标记为used（页表、kernel text/data、stack）。保留uefi map有个好处是memblock本身的内存目前是kernel data静态分配的，如果内存分段过多，导致memblock静态空间不够用，理论上可以在buddy system建立起来后再添加剩余的region。
- kernel mm: 建立 buddy system, 使用memblock系统来分配内存; 建立slab。
- setup idt：创建IDT。至此，所有内存都被kernel的内存管理系统自己管理。

### 早期内存管理
加载内核后，内存管理需要从固件转移到kernel。
而建立完整的内存管理系统的过程本身也需要内存分配回收，所以需要一个过渡的内存管理系统。
不想太依赖UEFI的功能，所以setup了过渡的memblock系统。（TODO memblock的必要性）

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

| 模块         | 条目                          | 分配     | 初始化                                  | 释放                                            |
| memblock     | ALL_MEMBLOCKS, USED_MEMBLOCKS | binary   | boot & vmkernel: generate from uefi map | page allocatorsetup之后就没用了，但没释放。leak |
| buddy system | MEM_ZONE                      | binary   |                                         | 不释放，一直存在                                |
| buddy system | MEM_ZONE.mem_map (Page frams) | memblock | physical.rs                             | 不释放，一直存在                                |

### 切换页表，切换内核栈
配置好页表后，更新`cr3`寄存器以切换到新的页表。
在切换页表前，先通过mmemblock分配器分配新的内核栈空间，在切换页表后立即切换到新内核栈。
切换页表和切换内核栈之间不要有任何栈操作，因为我们新的页表中没有映射原来的内核栈。

TODO：本节内容整理

## 内存模型
### 物理内存模型
flat模型

### Virtual Memory Layout
我们的kernel需要构建自己的页表。我们目前只支持4级页表，页大小为4K。
| Start addr       | Offset     | End addr         | Size  | VM area description                                                                          |
|------------------|------------|------------------|-------|----------------------------------------------------------------------------------------------|
| ffff888000000000 | -119.5  TB | ffffc87fffffffff | 64 TB | direct mapping of all physical memory (page_offset_base)                                     |
| ffffffff80000000 |            |                  |       | kernel text                                                                                  |
| BY-UEFI-FIRMWARE |            |                  |       | kernel text and data loaded by uefi firmware. identity mapped. Deleted after vmkernel start. |

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

## kernel的内存管理

### 物理内存和页分配器
参考linux的物理内存管理，最简实现：
使用flat物理内存模型，使用page frame数组管理所有物理内存，所有物理内存归属于同一个zone（MEM_ZONE）。
采用buddy system的设计，分配和释放都以2^order大小的连续页面为单位。
不同于linux下的实现，没有使用bitmap来记录buddy的分配情况，而是把页面所属的order直接保存在了Page结构体中。
看到一个patch, linux kernel早期也有人尝试过不使用bitmap：<https://linux.kernel.narkive.com/eRSCX6sn/rfc-buddy-allocator-without-bitmap-2-0-3>。

### 虚拟内存
kmalloc
vmalloc
