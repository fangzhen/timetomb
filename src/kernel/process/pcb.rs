//! Process Control Block (PCB) implementation
//!
//! The PCB contains all the information needed to manage a process,
//! including its state, context, memory information, and scheduling data.

use crate::{
    arch::x86_64::{
        mm::{direct_map_l2p, direct_map_p2l, P2L_OFFSET_BASE, USER_STACK_OFFSET_BASE},
        process::{
            context::ProcessContext,
            context_switch::{kernel_process_entry_wrapper, user_process_entry_wrapper},
        },
        syscall::{pt_regs::PtRegs, DEFAULT_USER_FLAGS},
    },
    kernel::mm::{paging::INIT_PT_ADDR, physical},
};
use alloc::string::String;
use bitflags::bitflags;
use core::fmt;
use timetomb::{
    arch::x86_64::{
        ffi_shared::VMKERNEL_ENTRY_ADDRESS,
        mm::{add_page_mapping, addr_to_page_entries, memzero},
    },
    kernel::mm::{PhysicalAddr, PAGE_SIZE},
};

pub const KERNEL_STACK_PAGES: usize = 2;
pub const USER_STACK_PAGES: usize = 2;
/// Process identifier type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u64);

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PID({})", self.0)
    }
}

/// Process states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    /// Process is ready to run and waiting for CPU
    Ready,
    /// Process is currently running
    Running,
    /// Process has finished execution
    Terminated,
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessState::Ready => write!(f, "Ready"),
            ProcessState::Running => write!(f, "Running"),
            ProcessState::Terminated => write!(f, "Terminated"),
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct ProcessFlags: u32 {
        const KERNEL_THREAD = 0b00000001;
    }
}

/// Process Control Block - contains all process information
#[derive(Debug)]
pub struct ProcessControlBlock {
    /// Process identifier
    pub pid: ProcessId,
    /// Parent process identifier
    pub parent_pid: Option<ProcessId>,
    /// Current process state
    pub state: ProcessState,
    /// CPU context (registers, stack pointer, etc.)
    pub context: ProcessContext,
    /// Process name (for debugging)
    pub name: String,
    /// Process flags
    pub flags: ProcessFlags,
    /// memory info
    pub memory_info: MemoryInfo,
}

#[derive(Debug)]
pub struct MemoryInfo {
    pub kernel_stack_base: usize,
    pub user_stack_base: usize,
    pub user_stack_pages: usize,
}

impl ProcessControlBlock {
    /// Allocate stack space for the process
    fn allocate_stack(count: usize) -> Result<usize, &'static str> {
        let page = physical::allocate_pages(count).unwrap();
        let addr = physical::MEM_ZONE.page_ref_to_addr(page);
        Ok(addr + count * PAGE_SIZE)
    }

    /// Create a new PCB for a kernel process
    pub fn new_kernel(pid: ProcessId, entry_point: usize) -> Result<Self, &'static str> {
        let kernel_stack_base = Self::allocate_stack(KERNEL_STACK_PAGES).unwrap();
        let user_stack_base = 0;
        let flags = ProcessFlags::KERNEL_THREAD;

        let mm_info = MemoryInfo {
            kernel_stack_base,
            user_stack_base,
            user_stack_pages: 0,
        };

        let rip = kernel_process_entry_wrapper as usize;

        let context = ProcessContext::new(entry_point, rip, kernel_stack_base, 0, unsafe {
            INIT_PT_ADDR
        });
        Ok(Self {
            pid,
            parent_pid: None,
            state: ProcessState::Ready,
            context,
            name: alloc::format!("kernel_process_{}", pid.0),
            flags: flags,
            memory_info: mm_info,
        })
    }

    fn prepare_user_vm(user_stack_pages: usize) -> (PhysicalAddr, MemoryInfo) {
        fn alloc_pt() -> PhysicalAddr {
            let page = physical::allocate_pages(1).unwrap();
            let laddr = physical::MEM_ZONE.page_ref_to_addr(page);
            memzero(laddr, PAGE_SIZE);
            let paddr = physical::MEM_ZONE.page_ref_to_paddr(page);
            paddr
        }
        let new_pt_base = alloc_pt();
        // Add kernel direct map and kernel text to user pagetable.
        let direct_idx = P2L_OFFSET_BASE >> 39 & 0x1ff;
        let vmkernel_idx = VMKERNEL_ENTRY_ADDRESS >> 39 & 0x1ff;
        let user_pt = addr_to_page_entries(direct_map_p2l(new_pt_base));
        let kernel_pt = addr_to_page_entries(direct_map_p2l(unsafe { INIT_PT_ADDR }));
        user_pt[direct_idx] = kernel_pt[direct_idx];
        user_pt[vmkernel_idx] = kernel_pt[vmkernel_idx];

        let kernel_stack_base = Self::allocate_stack(KERNEL_STACK_PAGES).unwrap();
        let user_stack_base = Self::allocate_stack(USER_STACK_PAGES).unwrap();
        let us_physical = direct_map_l2p(user_stack_base);
        let mm_info = MemoryInfo {
            kernel_stack_base,
            user_stack_base,
            user_stack_pages,
        };

        for i in 0..user_stack_pages {
            let laddr = USER_STACK_OFFSET_BASE - PAGE_SIZE * (i + 1);
            let paddr = us_physical - PAGE_SIZE * (i + 1);
            add_page_mapping(
                &mut || alloc_pt(),
                direct_map_p2l,
                laddr,
                paddr,
                new_pt_base,
            );
        }
        return (new_pt_base, mm_info);
    }

    /// Create a new PCB for a user process
    pub fn new_user(pid: ProcessId, entry_point: usize) -> Result<Self, &'static str> {
        let (pt_base, mm_info) = Self::prepare_user_vm(2);
        let rip = user_process_entry_wrapper as usize;
        let mut context = ProcessContext::new(
            entry_point,
            rip,
            mm_info.kernel_stack_base,
            USER_STACK_OFFSET_BASE,
            pt_base,
        );

        let child_pt_regs_addr = mm_info.kernel_stack_base - size_of::<PtRegs>();
        let child_pt_regs = unsafe { (child_pt_regs_addr as *mut PtRegs).as_mut().unwrap() };
        *child_pt_regs = PtRegs::default();
        child_pt_regs.rcx = entry_point as u64; // user entry point
        child_pt_regs.rdx = USER_STACK_OFFSET_BASE as u64;
        child_pt_regs.r11 = DEFAULT_USER_FLAGS;

        context.r13 = child_pt_regs_addr as u64; //  pt_regs
        context.rsp = child_pt_regs_addr as u64; // kernel stack

        let flags = ProcessFlags::empty();
        Ok(Self {
            pid,
            parent_pid: None,
            state: ProcessState::Ready,
            context,
            name: alloc::format!("user_process_{}", pid.0),
            flags: flags,
            memory_info: mm_info,
        })
    }

    /// Create a new PCB by forking from an existing process
    pub fn fork_from(
        child_pid: ProcessId,
        parent: &ProcessControlBlock,
    ) -> Result<Self, &'static str> {
        let user_stack_pages = parent.memory_info.user_stack_pages;
        let user_stack_size = user_stack_pages * PAGE_SIZE;
        let (pt_base, mm_info) = Self::prepare_user_vm(user_stack_pages);

        // copy parent user stack to child stack.
        unsafe {
            core::ptr::copy(
                (parent.memory_info.user_stack_base - user_stack_size) as *const u8,
                (mm_info.user_stack_base - user_stack_size) as *mut u8,
                user_stack_size,
            );
        }

        // Clone the parent's context
        let mut child_context = parent.context.clone();
        child_context.cr3 = pt_base as u64;

        Ok(Self {
            pid: child_pid,
            parent_pid: Some(parent.pid),
            state: ProcessState::Ready,
            context: child_context,
            name: alloc::format!("forked_from_{}", parent.pid.0),
            flags: parent.flags,
            memory_info: mm_info,
        })
    }
}
