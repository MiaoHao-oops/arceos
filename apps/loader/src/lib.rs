#![feature(asm_const)]
#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
extern crate axlog;

extern crate alloc;
extern crate axlibc;
use core::mem::size_of;
use core::ffi::{
    c_int,
    c_char,
};
use alloc::vec::Vec;
use axhal::mem::MemRegionFlags;
use elf::{phdr::*, sym::*, Elf64Ehdr, Elf64Rela, Elf64Sym};

use axalloc::global_allocator;
use axconfig::PHYS_VIRT_OFFSET;
use axhal::paging::{PageTable, MappingFlags};
use axstd::println;
use axtask::*;

const PLASH_START: usize = 0x2200_0000 + PHYS_VIRT_OFFSET;

struct ImgHeader {
    app_num: usize,
}

struct AppHeader {
    app_size: usize,
}

const IMG_HEADER_SIZE: usize = size_of::<ImgHeader>();
const APP_HEADER_SIZE: usize = size_of::<AppHeader>();

//
// App address space
//

fn init_app_page_table() -> PageTable {
    let mut page_table = PageTable::try_new().unwrap();
    // 0xffff_ffc0_8000_0000..0xffff_ffc0_c000_0000, VRWX_GAD, 1G block
    page_table.map_region(
        0xffff_ffc0_8000_0000.into(),
        0x8000_0000.into(),
        0x4000_0000,
        MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
        true
    );

    debug!("root paddr: {:x}", page_table.root_paddr());

    page_table
}

fn libc_start_main(main: fn(argc: c_int, argv: &&c_char)->c_int) {
    // TODO: pass argc and argv to main
    axtask::exit(main(0, &&0));
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn load_elf(app_num: usize, load_start: usize) -> (usize, AxTaskRef) {

    let app_header: &AppHeader = unsafe {
        &*((PLASH_START + IMG_HEADER_SIZE + app_num * APP_HEADER_SIZE) as *const AppHeader)
    };
    let load_size = app_header.app_size;
    info!("hello app ELF size: {} bytes", load_size);

    let mut page_table = init_app_page_table();

    // Grab ELF header
    let ehdr = unsafe { &*(load_start as *const Elf64Ehdr) };

    // Grab program header table, and load PT_LOAD segments into memory
    // TODO: dynamically page set flags
    let pht = ehdr.get_pht(load_start);
    for phe in pht {
        if phe.p_type == PT_LOAD {
            info!("loading segment to vaddr {:x}, mem size: {:x}", phe.p_vaddr, phe.p_memsz);
            let begin_va = phe.p_vaddr as usize;
            let end_va = begin_va + phe.p_memsz as usize;
            let begin_aligned_va = begin_va >> 12 << 12;
            let end_aligned_va = (end_va + 4096) >> 12 << 12;
            let begin_offset = begin_va - begin_aligned_va;
            let num_pages = (end_aligned_va - begin_aligned_va) / 4096;
            let pages_kva = global_allocator().alloc_pages(num_pages, 4096).unwrap();
 
            let flags = MappingFlags::from_bits(
                ((phe.p_flags & PF_X) << 2 |
                (phe.p_flags & PF_W) |
                (phe.p_flags & PF_R) >> 2) as usize
            ).unwrap();
            let va = begin_aligned_va;
            let pa = pages_kva - PHYS_VIRT_OFFSET;
            info!("mapping va: {:x} to pa: {:x}", va, pa);
            page_table.map_region(
                va.into(),
                pa.into(),
                4096 * num_pages,
                flags,
                false
            );

            let clearer = unsafe {
                core::slice::from_raw_parts_mut(
                    pages_kva as *mut u8,
                    4096 * num_pages
                )
            };
            clearer.fill(0);
            let load_bin = unsafe {
                core::slice::from_raw_parts(
                    (load_start + phe.p_offset as usize) as *const u8,
                    phe.p_filesz as usize,
                )
            };
            let load_dest = unsafe {
                core::slice::from_raw_parts_mut(
                    (pages_kva + begin_offset) as *mut u8,
                    phe.p_filesz as usize,
                )
            };
            load_dest.copy_from_slice(load_bin);
        }
    }

    // execute dynamic link
    let rela_plt_hdr = ehdr.get_she(load_start, ".rela.plt").unwrap();
    let rela_dyn_hdr = ehdr.get_she(load_start, ".rela.dyn").unwrap();
    let dynsym_hdr = ehdr.get_she(load_start, ".dynsym").unwrap();
    let dynstr_hdr = ehdr.get_she(load_start, ".dynstr").unwrap();
    let dynsyms = dynsym_hdr.get_table::<Elf64Sym>(load_start);

    // linking external symbols
    let rela_plte = rela_plt_hdr.get_table::<Elf64Rela>(load_start);
    for rela in rela_plte {
        let dynsym = &dynsyms[rela.r_sym() as usize];
        if dynsym.st_bind() == STB_GLOBAL && dynsym.st_type() == STT_FUNC {
            let func_name = dynstr_hdr.get_name(load_start, dynsym.st_name);
            if let Some(link_vaddr) = find_link_vaddr(func_name) {
                let (pa, _, _) = page_table.query((rela.r_offset as usize).into()).unwrap();
                let pa: usize = pa.into();
                let kva = pa + PHYS_VIRT_OFFSET;
                let link_dest = unsafe {
                    &mut *(kva as *mut usize)
                };
                *link_dest = link_vaddr;
                info!("link `{}` to {:x}", func_name, link_vaddr);
            } else {
                panic!("link function name [{}] not found!", func_name);
            }
        }
    }

    // linking internal symbols
    let rela_dyne = rela_dyn_hdr.get_table::<Elf64Rela>(load_start);
    for rela in rela_dyne {
        let link_vaddr = rela.r_addend as usize;
        let (pa, _, _) = page_table.query((rela.r_offset as usize).into()).unwrap();
        let pa: usize = pa.into();
        let kva = pa + PHYS_VIRT_OFFSET;
        let link_dest = unsafe {
            &mut *(kva as *mut usize)
        };
        *link_dest = link_vaddr;
        info!(
            "ptr storage vaddr: {:x}, link vaddr: {:x}",
            rela.r_offset, link_vaddr
        );
    }

    info!("Execute app ...");
    info!("app entry point: {:x}", ehdr.e_entry);

    let page_table_pa: usize = page_table.root_paddr().into();
    info!("set page table, pa: {:x}", page_table_pa);
    let satp = (8 << 60) | (0 << 44) | (page_table_pa >> 12);
    let inner = spawn_ptr(
        ehdr.e_entry as usize,
        "hello".into(),
        4096,
        satp,
        page_table
    );
    (load_size, inner)
}

///
/// dynamic link function table
/// 

extern "C" {
    fn putchar();
    fn printf();
    fn puts();
    fn malloc();
    fn free();
    fn pthread_self();
    fn pthread_exit();
    fn pthread_mutex_unlock();
    fn __assert_fail();
    fn sprintf();
    fn getpid();
    fn pthread_create();
    fn pthread_mutex_lock();
    fn pthread_join();
    fn rand();
    fn calloc();
    // fn send();
    // fn recv();
    // fn socket();
    // fn memcpy();
    // fn strlen();
    // fn close();
    // fn snprintf();
    // fn perror();
    // fn listen();
    // fn bind();
    // fn accept();
    // fn htons();
    // fn memset();
    // fn inet_pton();
    // fn sendto();
    // fn recvfrom();
    // fn ntohs();
    // fn freeaddrinfo();
    // fn connect();
    // fn inet_ntop();
    // fn getaddrinfo();
}

static mut FUNC_TABLE: [(&str, usize); 17] = [
    ("__libc_start_main", 0),
    ("putchar", 0),
    ("printf", 0),
    ("puts", 0),
    ("malloc", 0),
    ("free", 0),
    ("pthread_self", 0),
    ("pthread_exit", 0),
    ("pthread_mutex_unlock", 0),
    ("__assert_fail", 0),
    ("sprintf", 0),
    ("getpid", 0),
    ("pthread_create", 0),
    ("pthread_mutex_lock", 0),
    ("pthread_join", 0),
    ("rand", 0),
    ("calloc", 0),
    // ("send", 0),
    // ("recv", 0),
    // ("socket", 0),
    // ("memcpy", 0),
    // ("strlen", 0),
    // ("close", 0),
    // ("snprintf", 0),
    // ("perror", 0),
    // ("listen", 0),
    // ("bind", 0),
    // ("accept", 0),
    // ("htons", 0),
    // ("memset", 0),
    // ("inet_pton", 0),
    // ("sendto", 0),
    // ("recvfrom", 0),
    // ("ntohs", 0),
    // ("freeaddrinfo", 0),
    // ("connect", 0),
    // ("inet_ntop", 0),
    // ("getaddrinfo", 0),
];

fn init_func_table() {
    unsafe {
        FUNC_TABLE[0].1 = libc_start_main as usize;
        FUNC_TABLE[1].1 = putchar as usize;
        FUNC_TABLE[2].1 = printf as usize;
        FUNC_TABLE[3].1 = puts as usize;
        FUNC_TABLE[4].1 = malloc as usize;
        FUNC_TABLE[5].1 = free as usize;
        FUNC_TABLE[6].1 = pthread_self as usize;
        FUNC_TABLE[7].1 = pthread_exit as usize;
        FUNC_TABLE[8].1 = pthread_mutex_unlock as usize;
        FUNC_TABLE[9].1 = __assert_fail as usize;
        FUNC_TABLE[10].1 = sprintf as usize;
        FUNC_TABLE[11].1 = getpid as usize;
        FUNC_TABLE[12].1 = pthread_create as usize;
        FUNC_TABLE[13].1 = pthread_mutex_lock as usize;
        FUNC_TABLE[14].1 = pthread_join as usize;
        FUNC_TABLE[15].1 = rand as usize;
        FUNC_TABLE[16].1 = calloc as usize;
        // FUNC_TABLE[6].1 = send as usize;
        // FUNC_TABLE[7].1 = recv as usize;
        // FUNC_TABLE[8].1 = socket as usize;
        // FUNC_TABLE[9].1 = memcpy as usize;
        // FUNC_TABLE[10].1 = strlen as usize;
        // FUNC_TABLE[11].1 = close as usize;
        // FUNC_TABLE[12].1 = snprintf as usize;
        // FUNC_TABLE[13].1 = perror as usize;
        // FUNC_TABLE[14].1 = listen as usize;
        // FUNC_TABLE[15].1 = bind as usize;
        // FUNC_TABLE[16].1 = accept as usize;
        // FUNC_TABLE[17].1 = htons as usize;
        // FUNC_TABLE[18].1 = memset as usize;
        // FUNC_TABLE[19].1 = inet_pton as usize;
        // FUNC_TABLE[20].1 = sendto as usize;
        // FUNC_TABLE[21].1 = recvfrom as usize;
        // FUNC_TABLE[22].1 = ntohs as usize;
        // FUNC_TABLE[23].1 = freeaddrinfo as usize;
        // FUNC_TABLE[24].1 = connect as usize;
        // FUNC_TABLE[25].1 = inet_ntop as usize;
        // FUNC_TABLE[26].1 = getaddrinfo as usize;
    }
}

fn find_link_vaddr(func_name: &str) -> Option<usize> {
    unsafe {
        for (name, vaddr) in FUNC_TABLE {
            if func_name == name {
                return Some(vaddr);
            }
        }
    }
    None
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    init_func_table();

    let img_header: &ImgHeader = unsafe { &*(PLASH_START as *const ImgHeader) };
    let app_num = img_header.app_num;
    let mut load_start = PLASH_START + IMG_HEADER_SIZE + app_num * APP_HEADER_SIZE;

    info!("Load payload ...");
    info!("putchar address: {:x}", putchar as usize);

    let mut tasks = Vec::new();

    for i in 0..app_num {
        let (load_size, inner) = load_elf(i, load_start);
        load_start += load_size;
        tasks.push(inner);
    }

    tasks.into_iter().for_each(|t| { t.join(); });
}
