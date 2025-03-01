use core::{
    cell::{Ref, RefCell, RefMut},
    cmp::min,
    slice::{from_raw_parts, from_raw_parts_mut},
};
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use timetomb::{
    arch::x86_64::mm::{l2p, p2l},
    kernel::mm::{
        memblock::{self, MemblockType},
        LinearAddr, PhysicalAddr, PAGE_SIZE,
    },
};

use crate::library::bitops;

use super::slab::KmemCache;

pub const MAX_ORDER: usize = 10;

pub struct Zone {
    data: RefCell<ZoneData>,
}
unsafe impl Sync for Zone {}

#[derive(Debug)]
pub struct ZoneData {
    mem_map_addr: usize,
    page_len: usize,
    pub free_lists: Option<[LinkedList<PageBuddyAdapter>; MAX_ORDER + 1]>,
}

pub static MEM_ZONE: Zone = Zone {
    data: RefCell::new(ZoneData {
        free_lists: None,
        mem_map_addr: 0,
        page_len: 0,
    }),
};
impl ZoneData {
    /// Convert a memory block to Page struct array.
    /// No check if pages_addr is valid. The caller should ensure that.
    fn init_data(&mut self, pages_addr: LinearAddr, len: usize) {
        self.mem_map_addr = pages_addr;
        self.page_len = len;
        self.free_lists = Some(core::array::from_fn(|_| {
            LinkedList::new(PageBuddyAdapter::new())
        }));
        let pages = unsafe { from_raw_parts_mut(self.mem_map_addr as *mut Page, self.page_len) };
        for p in pages {
            *p = Page::new();
        }
    }
    fn get_mem_map(&self) -> &'static [Page] {
        unsafe { from_raw_parts(self.mem_map_addr as *const Page, self.page_len) }
    }
    fn get_free_list_mut(&mut self) -> &mut [LinkedList<PageBuddyAdapter>; MAX_ORDER + 1] {
        return self.free_lists.as_mut().unwrap();
    }
    fn page_ref_to_pfn(&self, p: &Page) -> usize {
        return ((p as *const Page as usize) - self.mem_map_addr) / size_of::<Page>();
    }
    fn pfn_to_page_ref(&self, pfn: usize) -> &'static Page {
        let page_addr = self.mem_map_addr + pfn * size_of::<Page>();
        return unsafe { (page_addr as *const Page).as_ref().unwrap() };
    }
    fn find_buddy(&self, p: &Page, order: usize) -> &'static Page {
        let pfn = self.page_ref_to_pfn(p);
        let buddy_pfn = find_buddy_pfn(pfn, order);
        self.pfn_to_page_ref(buddy_pfn)
    }
    fn find_parent(&self, p: &Page, order: usize) -> &'static Page {
        let pfn = self.page_ref_to_pfn(p);
        let parent_pfn = find_parent_pfn(pfn, order - 1);
        self.pfn_to_page_ref(parent_pfn)
    }

    fn split_buddy(&mut self, page: &'static Page, order: usize) {
        let buddy = self.find_buddy(page, order);
        let free_lists = self.get_free_list_mut();
        page.add_to_freelist(free_lists, order);
        buddy.add_to_freelist(free_lists, order);
    }
    fn allocate_pages(&mut self, order: usize) -> Option<&'static Page> {
        if order > MAX_ORDER {
            return None;
        }
        let free_lists = self.get_free_list_mut();
        if free_lists[order].is_empty() {
            if let Some(page) = self.allocate_pages(order + 1) {
                self.split_buddy(page, order);
            } else {
                return None;
            }
        }
        let free_lists = self.get_free_list_mut();
        let page = free_lists[order].pop_front();
        let first_page = page.unwrap();
        first_page.set_allocated();
        return page;
    }
}

impl Zone {
    fn get_inner(&self) -> Ref<ZoneData> {
        self.data.borrow()
    }
    fn get_inner_mut(&self) -> RefMut<ZoneData> {
        self.data.borrow_mut()
    }
    pub fn page_ref_to_paddr(&self, p: &Page) -> PhysicalAddr {
        return self.page_ref_to_pfn(p) * PAGE_SIZE;
    }
    pub fn page_ref_to_addr(&self, p: &Page) -> LinearAddr {
        return p2l(self.page_ref_to_pfn(p) * PAGE_SIZE);
    }
    pub fn addr_to_page_ref(&self, addr: LinearAddr) -> &'static Page {
        return self.pfn_to_page_ref(paddr_to_pfn(l2p(addr)));
    }
    pub fn page_ref_to_pfn(&self, p: &Page) -> usize {
        self.get_inner().page_ref_to_pfn(p)
    }
    pub fn pfn_to_page_ref(&self, pfn: usize) -> &'static Page {
        self.get_inner().pfn_to_page_ref(pfn)
    }
    pub fn init_page_allocator(&self, all_mb: &MemblockType, used_mb: &MemblockType) {
        log::info!("Init page allocator with memblock regions.");
        let all_regions = all_mb.regions;
        let last_region = all_regions[all_mb.cnt - 1];
        let min_addr = all_regions[0].start;
        let sfn = min_addr / PAGE_SIZE;
        let max_addr = last_region.start + last_region.size;
        let len = (max_addr - min_addr) / PAGE_SIZE;
        let pages_addr = memblock::allocate_memory(0, size_of::<Page>() * len, PAGE_SIZE, 0);
        let mut zm = MEM_ZONE.get_inner_mut();
        zm.init_data(pages_addr, len);
        let pages = zm.get_mem_map();
        let used_regions = used_mb.regions;
        for i in 0..all_mb.cnt - 1 {
            let r1 = all_regions[i];
            let r2 = all_regions[i + 1];
            let idx = paddr_to_pfn(r1.start + r1.size) - sfn;
            let l = (r2.start - r1.start - r1.size) / PAGE_SIZE;
            for i in idx..idx + l {
                pages[i].get_inner_mut().flags = page_flag::NO_PHYSICAL | page_flag::OCCUPIED;
            }
        }
        for i in 0..used_mb.cnt {
            let r = used_regions[i];
            let idx = paddr_to_pfn(r.start) - sfn;
            let l = r.size / PAGE_SIZE;
            for i in idx..idx + l {
                pages[i].get_inner_mut().flags = page_flag::OCCUPIED;
            }
        }
        log::info!("Setup buddy system free lists");
        let free_lists = zm.free_lists.as_mut().unwrap();
        let mut add_to_list = |mut start: usize, end: usize| {
            log::info!("Memory region start: {:#x}, end: {:#x}", start, end);
            while start < end {
                let mut order = min(MAX_ORDER, bitops::ffs(start));
                while start + (1 << order) > end {
                    order -= 1;
                }
                pages[start].add_to_freelist(free_lists, order);
                start += 1 << order;
            }
        };

        let all_regions = all_mb.regions;
        let used_regions = used_mb.regions;
        let mut idu = 0;
        for i in 0..all_mb.cnt {
            let r1 = all_regions[i];
            let mut start = r1.start;
            let r1_end = start + r1.size;
            while idu < used_mb.cnt {
                let r2 = used_regions[idu];
                if r1_end > r2.start {
                    add_to_list(paddr_to_pfn(start + PAGE_SIZE - 1), paddr_to_pfn(r2.start));
                    start = r2.start + r2.size;
                    idu += 1;
                } else {
                    add_to_list(paddr_to_pfn(start + PAGE_SIZE - 1), paddr_to_pfn(r1_end));
                    break;
                }
            }
        }
    }
    pub fn print_buddy_status(&self) {
        let zm = self.get_inner();
        let page_base = zm.mem_map_addr;
        let free_lists = &mut zm.free_lists.as_ref().unwrap();
        for i in 0..MAX_ORDER + 1 {
            log::info!("Free list of order {}", i);
            let list = &free_lists[i];
            for i in list.iter() {
                log::info!(
                    "PFN: {:#x}",
                    ((i as *const Page as usize) - page_base) / size_of::<Page>()
                );
            }
        }
    }
    pub fn allocate_pages(&self, order: usize) -> Option<&'static Page> {
        if order > MAX_ORDER {
            return None;
        }
        let mut zm = self.get_inner_mut();
        return zm.allocate_pages(order);
    }
    pub fn free_pages(&self, page: &'static Page) {
        let mut zm = self.get_inner_mut();
        let free_lists = zm.free_lists.as_mut().unwrap();
        let order = page.get_inner().order;
        page.add_to_freelist(free_lists, order);
        self.merge_buddy(&mut zm, page);
    }

    fn merge_buddy(&self, zm: &mut RefMut<ZoneData>, page: &Page) {
        let page_order = page.get_inner().order;
        let buddy = zm.find_buddy(page, page_order);
        if buddy.get_inner().flags & page_flag::OCCUPIED != 0 {
            return;
        }
        if page_order >= MAX_ORDER {
            return;
        }
        if page_order != buddy.get_inner().order {
            return;
        }
        let free_lists = zm.free_lists.as_mut().unwrap();
        let mut curor = unsafe { free_lists[page_order].cursor_mut_from_ptr(page as *const _) };
        curor.remove();
        let mut curor = unsafe { free_lists[page_order].cursor_mut_from_ptr(buddy as *const _) };
        curor.remove();
        let parent = zm.find_parent(page, page_order);

        let free_lists = zm.free_lists.as_mut().unwrap();
        parent.add_to_freelist(free_lists, page_order + 1);

        self.merge_buddy(zm, parent);
    }
}

pub mod page_flag {
    pub const NO_PHYSICAL: u32 = 1;
    pub const OCCUPIED: u32 = 1 << 1;
}

unsafe impl Sync for Page {}
#[derive(Debug, Clone)]
pub struct Page {
    pub data: RefCell<PageData>,
    pub buddy_link: LinkedListLink, // free list of buddy system
}
#[derive(Debug, Clone)]
//TODO(fangzhen) some field could share memory
pub struct PageData {
    pub flags: u32,
    pub order: usize,

    // slab fields
    pub slab_next: Option<&'static Page>, // partial list of slab system
    pub free_obj_head: usize,             // Address of next available object
    pub used: usize,                      // used objects of this slab
    pub kmem_cache: Option<&'static KmemCache>,
    pub compound_head: Option<&'static Page>, // head page of this buddy block
}

intrusive_adapter!(pub PageBuddyAdapter = &'static Page: Page {buddy_link: LinkedListLink});

impl Page {
    pub fn get_inner(&self) -> Ref<PageData> {
        self.data.borrow()
    }
    pub fn get_inner_mut(&self) -> RefMut<PageData> {
        self.data.borrow_mut()
    }

    pub const fn new() -> Page {
        Page {
            buddy_link: LinkedListLink::new(),
            data: RefCell::new(PageData {
                flags: 0,
                order: 0,
                slab_next: None,
                free_obj_head: 0,
                used: 0,
                kmem_cache: None,
                compound_head: None,
            }),
        }
    }
    pub fn add_to_freelist(
        &'static self,
        free_lists: &mut [LinkedList<PageBuddyAdapter>; MAX_ORDER + 1],
        order: usize,
    ) {
        let mut p = self.get_inner_mut();
        p.order = order;
        p.flags = p.flags & (!page_flag::OCCUPIED);
        free_lists[order].push_front(self);
    }

    fn page_to_slice(&self, order: usize) -> &'static [Page] {
        return unsafe { from_raw_parts(self as *const Page, 1 << order) };
    }
    pub fn set_allocated(&'static self) {
        self.get_inner_mut().flags |= page_flag::OCCUPIED;
        let order = self.get_inner().order;
        for p in self.page_to_slice(order) {
            p.get_inner_mut().compound_head = Some(self);
        }
    }
}

pub fn paddr_to_pfn(addr: PhysicalAddr) -> usize {
    return addr / PAGE_SIZE;
}

fn find_buddy_pfn(pfn: usize, order: usize) -> usize {
    return pfn ^ (1 << order);
}

fn find_parent_pfn(pfn: usize, order: usize) -> usize {
    return pfn & (!(1 << order));
}

///Construcct buddy system from memblock regions.
pub fn init_page_allocator(all_mb: &MemblockType, used_mb: &MemblockType) {
    MEM_ZONE.init_page_allocator(all_mb, used_mb);
}

pub fn allocate_pages(order: usize) -> Option<&'static Page> {
    return MEM_ZONE.allocate_pages(order);
}

pub fn free_pages(page: &'static Page) {
    MEM_ZONE.free_pages(page);
}
