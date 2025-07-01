use core::{
    array,
    cell::{RefCell, RefMut},
};

use super::physical;
use crate::library::{
    bitops::{self, align_floor},
    rust::SyncOnceUnsafeCell,
};
use timetomb::kernel::mm::PAGE_SIZE;

#[derive(Debug)]
pub struct KmemCache {
    data: RefCell<KmemCacheData>,
}

unsafe impl Sync for KmemCache {}

pub static KMEM_CACHE: KmemCache = KmemCache::new();

#[derive(Debug, Clone, Copy)]
pub struct KmemCacheData {
    object_size: usize,
    size: usize,                              // size after align
    count: usize,                             // number of objects per slab
    order: usize,                             // order of slab page in buddy system
    free_list: usize,                         // Address of next available object
    page: Option<&'static physical::Page>,    // first page of current slab
    partial: Option<&'static physical::Page>, // list of partial slabs
    name: &'static str,                       //slab name, encoded in utf-8
}
impl KmemCache {
    fn get_inner_mut(&self) -> RefMut<KmemCacheData> {
        self.data.borrow_mut()
    }
    const fn new() -> KmemCache {
        KmemCache {
            data: RefCell::new(KmemCacheData {
                object_size: 0,
                size: 0,
                count: 0,
                order: 0,
                free_list: 0,
                page: None,
                partial: None,
                name: "",
            }),
        }
    }
    pub fn init(&'static self, obj_size: usize, order: usize, name: &'static str) {
        let mut s = self.get_inner_mut();
        log::info!("Creating slab: name: {}, object size: {}", name, obj_size);
        s.object_size = obj_size;
        s.order = order;
        s.name = name;
        s.allocate_slab_page(self);
    }
    pub fn allocate_object(&'static self) -> usize {
        // current page is full, update current page from partial list or buddy system
        let mut s = self.get_inner_mut();
        let page = s.page.unwrap();
        let used = page.get_inner().used;
        if s.count == used {
            page.get_inner_mut().free_obj_head = 0;
            match s.partial {
                None => s.allocate_slab_page(self),
                Some(partial) => {
                    s.free_list = partial.get_inner().free_obj_head;
                    s.page = s.partial;
                    s.partial = partial.get_inner().slab_next;
                }
            }
        }
        let obj = s.free_list;
        s.free_list = unsafe { *(obj as *const usize) };
        s.page.unwrap().get_inner_mut().used += 1;
        return obj;
    }

    pub fn free_object(&self, obj_addr: usize) {
        let mut s = self.get_inner_mut();
        let current_page_addr = physical::MEM_ZONE.page_ref_to_addr(s.page.unwrap());
        let current_slab_end = current_page_addr + PAGE_SIZE * (1 << s.order);
        if obj_addr >= current_page_addr && obj_addr < current_slab_end {
            // The freed object is on current slab page.
            unsafe { *(obj_addr as *mut usize) = s.free_list };
            s.free_list = obj_addr;
        } else {
            let obj_page_addr = align_floor(obj_addr, PAGE_SIZE * (1 << s.order));
            let obj_page = physical::MEM_ZONE.addr_to_page_ref(obj_page_addr);
            if obj_page.get_inner().used == s.count {
                // full slab become partial
                obj_page.get_inner_mut().slab_next = s.partial;
                s.partial = Some(obj_page);
            }
            // link freed object to free list
            unsafe { *(obj_addr as *mut usize) = obj_page.get_inner().free_obj_head };
            obj_page.get_inner_mut().free_obj_head = obj_addr;
            obj_page.get_inner_mut().used -= 1;

            // return empty page back to buddy system
            if obj_page.get_inner().used == 0 {
                physical::free_pages(obj_page);
            }
        }
    }
}
impl KmemCacheData {
    fn calculate_size(&mut self) {
        let ptr_size = size_of::<*const usize>();
        self.size = bitops::align_ceil(self.object_size, ptr_size);
        self.count = PAGE_SIZE * (1 << self.order) / self.size;
    }
    fn init_object(&mut self, page: &'static physical::Page) {
        let page_addr = physical::MEM_ZONE.page_ref_to_addr(page);
        self.free_list = page_addr;
        page.get_inner_mut().used = 0;
        self.page = Some(page);
        let mut p = page_addr;
        for _ in 0..self.count - 1 {
            unsafe { *(p as *mut usize) = p + self.size };
            p += self.size;
        }
        unsafe { *(p as *mut usize) = 0 };
    }
    fn allocate_slab_page(&mut self, s: &'static KmemCache) {
        self.calculate_size();
        let page = physical::allocate_pages(self.order);
        if page.is_none() {
            panic!("Failed to allocate page for slab cache");
        }
        log::info!(
            "Allocated page PFN:{:#x} for slab",
            physical::MEM_ZONE.page_ref_to_pfn(page.unwrap())
        );
        let page = page.unwrap();
        page.get_inner_mut().kmem_cache = Some(s);
        self.init_object(page);
    }
}

pub fn init_slab() {
    log::info!("Init slab allocator");
    KMEM_CACHE.init(size_of::<KmemCache>(), 0, "kmem_cache");
}

pub fn kmem_cache_create(obj_size: usize, order: usize, name: &'static str) -> &'static KmemCache {
    let kmem_cache_addr = KMEM_CACHE.allocate_object();
    let kmem_cache = unsafe { &mut *(kmem_cache_addr as *mut KmemCache) };
    *kmem_cache = KmemCache::new();
    kmem_cache.init(obj_size, order, name);
    return kmem_cache;
}

struct KmallocSlabInfo {
    size: usize,
    order: usize,
    name: &'static str,
}

//TODO These consts are related.
const KMALLOC_SLAB_MAX: usize = 4096;
const KMALLOC_SLAB_COUNT: usize = 10;
const KMALLOC_SLAB_INFO: [KmallocSlabInfo; KMALLOC_SLAB_COUNT] = [
    KmallocSlabInfo {
        size: 8,
        order: 0,
        name: "kmalloc-8",
    },
    KmallocSlabInfo {
        size: 16,
        order: 0,
        name: "kmalloc-16",
    },
    KmallocSlabInfo {
        size: 32,
        order: 0,
        name: "kmalloc-32",
    },
    KmallocSlabInfo {
        size: 64,
        order: 0,
        name: "kmalloc-64",
    },
    KmallocSlabInfo {
        size: 128,
        order: 0,
        name: "kmalloc-128",
    },
    KmallocSlabInfo {
        size: 256,
        order: 0,
        name: "kmalloc-256",
    },
    KmallocSlabInfo {
        size: 512,
        order: 0,
        name: "kmalloc-512",
    },
    KmallocSlabInfo {
        size: 1024,
        order: 0,
        name: "kmalloc-1k",
    },
    KmallocSlabInfo {
        size: 2048,
        order: 1,
        name: "kmalloc-2k",
    },
    KmallocSlabInfo {
        size: 4096,
        order: 2,
        name: "kmalloc-4k",
    },
];
static KMALLOC_SLABS: SyncOnceUnsafeCell<[&KmemCache; KMALLOC_SLAB_COUNT]> =
    SyncOnceUnsafeCell::new();

pub fn init_kmalloc() {
    log::info!("Init kmalloc");
    let slabs: [&KmemCache; KMALLOC_SLAB_COUNT] = array::from_fn(|i| {
        let info = &KMALLOC_SLAB_INFO[i];
        kmem_cache_create(info.size, info.order, info.name)
    });

    let _ = KMALLOC_SLABS.set(slabs);
}

fn size_to_slab_index(size: usize) -> usize {
    if size == 0 {
        return 0;
    }
    let pow = bitops::ffs(bitops::power_of_two_ceil(size));
    if pow <= 3 {
        return 0;
    } else {
        return pow - 3;
    }
}
pub fn kmalloc(size: usize) -> usize {
    if size <= KMALLOC_SLAB_MAX {
        let slab_index = size_to_slab_index(size);
        return KMALLOC_SLABS.get_unchecked()[slab_index].allocate_object();
    } else {
        let order = bitops::power_of_two_ceil(size) / PAGE_SIZE;
        let p = physical::allocate_pages(order);
        match p {
            None => 0,
            Some(p) => physical::MEM_ZONE.page_ref_to_addr(p),
        }
    }
}

pub fn kfree(addr: usize) {
    let page = physical::MEM_ZONE.addr_to_page_ref(addr);
    let first_page = page.get_inner().compound_head.unwrap();
    let slab = first_page.get_inner().kmem_cache;
    match slab {
        None => {
            let p = physical::MEM_ZONE.addr_to_page_ref(addr);
            physical::free_pages(p);
        }
        Some(s) => {
            s.free_object(addr);
        }
    }
}

pub fn test_slab() {
    log::info!("Test slab");
    let count = KMEM_CACHE.get_inner_mut().count * 2;
    let s = &KMEM_CACHE;
    log::info!("count: {}", count);
    let mut addrs = [0; 512];
    for i in 0..count {
        addrs[i] = s.allocate_object();
    }
    physical::MEM_ZONE.print_buddy_status();
    log::info!("Free objects: {:#x}", addrs[10]);
    s.free_object(addrs[10]);
    addrs[10] = s.allocate_object();
    log::info!("Allocated objects: {:#x}", addrs[10]);
    for i in 0..count {
        s.free_object(addrs[i]);
    }
    physical::MEM_ZONE.print_buddy_status();

    log::info!("test kmalloc");
    let a = kmalloc(7);
    log::info!("Allocated objects: {:#x}", a);
    kfree(a);

    let a = kmalloc(4);
    log::info!("Allocated objects: {:#x}", a);
    kfree(a);

    let a = kmalloc(0);
    log::info!("Allocated objects: {:#x}", a);
    kfree(a);

    let a = kmalloc(1);
    log::info!("Allocated objects: {:#x}", a);
    kfree(a);

    let a = kmalloc(2000);
    log::info!("Allocated objects: {:#x}", a);
    let b = kmalloc(2048);
    log::info!("Allocated objects: {:#x}", b);
    kfree(a);
    kfree(b);

    let a = kmalloc(2000);
    log::info!("Allocated objects: {:#x}", a);
    let b = kmalloc(2048);
    log::info!("Allocated objects: {:#x}", b);
    kfree(a);
    kfree(b);

    let a = kmalloc(4097);
    log::info!("Allocated objects: {:#x}", a);
    kfree(a);
}
