#![feature(asm_const)]
#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

extern crate alloc;
extern crate axlibc;
use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::println;
use elf::{phdr::*, sym::*, Elf64Ehdr, Elf64Rela, Elf64Sym};

const PLASH_START: usize = 0x2200_0000;

struct ImgHeader {
    app_num: usize,
}

struct AppHeader {
    app_size: usize,
}

const IMG_HEADER_SIZE: usize = size_of::<ImgHeader>();
const APP_HEADER_SIZE: usize = size_of::<AppHeader>();

extern "C" {
    fn putchar(c: i32) -> i32;
    fn printf();
}

//
// App address space
//

#[link_section = ".data.app_page_table"]
static mut APP_PT_PGD: [u64; 512] = [0; 512];
#[link_section = ".data.app_page_table"]
static mut APP_PT_PMD0: [u64; 512] = [0; 512];
#[link_section = ".data.app_page_table"]
static mut APP_PT_PTD0: [u64; 512] = [0; 512];

fn init_app_page_table() {
    unsafe {
        let pmd0_pa = APP_PT_PMD0.as_ptr() as usize - axconfig::PHYS_VIRT_OFFSET;
        let ptd0_pa = APP_PT_PTD0.as_ptr() as usize - axconfig::PHYS_VIRT_OFFSET;
        // 0x8000_0000..0xc000_0000, VRWX_GAD, 1G block
        APP_PT_PGD[2] = (0x80000 << 10) | 0xef;
        // 0xffff_ffc0_8000_0000..0xffff_ffc0_c000_0000, VRWX_GAD, 1G block
        APP_PT_PGD[0x102] = (0x80000 << 10) | 0xef;

        // For MMIO space
        // 0xffff_ffc0_0000_0000..0xffff_ffc0_4000_0000, VRWX_GAD, 1G block
        APP_PT_PGD[0x100] = (0x00000 << 10) | 0xef;

        // For App aspace, using 3 level page table
        // _______V, points to APP_PT_PMD0
        APP_PT_PGD[0] = (pmd0_pa as u64 >> 12 << 10) | 0x01;
        // _______V, points to APP_PT_PTD0
        APP_PT_PMD0[0] = (ptd0_pa as u64 >> 12 << 10) | 0x01;
        // 0x0000_0000..0x0000_1000, DAG_X_RV, 4K page
        APP_PT_PTD0[0] = (0x80100 << 10) | 0xeb;
        // 0x0000_1000..0x0000_3000, DAG__WRV, 4K page
        APP_PT_PTD0[1] = (0x80101 << 10) | 0xe7;
        APP_PT_PTD0[2] = (0x80102 << 10) | 0xe7;
    }
}

unsafe fn aspace_save(pg_table_paddr: usize) -> usize {
    use riscv::register::satp;
    let prev_satp = satp::read().bits();
    let page_table_root = pg_table_paddr - axconfig::PHYS_VIRT_OFFSET;
    satp::set(satp::Mode::Sv39, 0, page_table_root >> 12);
    riscv::asm::sfence_vma_all();
    prev_satp
}

unsafe fn aspave_restore(pg_table_paddr: usize) {
    use riscv::register::satp;
    satp::write(pg_table_paddr);
    riscv::asm::sfence_vma_all();
}

fn libc_start_main(main: usize) {
    // TODO: make this function arch-irrelative
    unsafe {
        core::arch::asm!("
            mv  ra, s0
            jr  a0",
            in("a0") main,
        );
    }
}

fn load_elf(app_num: usize, load_start: usize) -> usize {
    // TODO: translate va2pa through page table
    fn va2pa(va: usize) -> usize {
        const POFFSET: usize = 0x8010_0000;
        va + POFFSET
    }

    let app_header: &AppHeader = unsafe {
        &*((PLASH_START + IMG_HEADER_SIZE + app_num * APP_HEADER_SIZE) as *const AppHeader)
    };
    let load_size = app_header.app_size;
    println!("hello app ELF size: {} bytes", load_size);
    println!("load_start: {:x}", load_start);

    // Grab ELF header
    let ehdr = unsafe { &*(load_start as *const Elf64Ehdr) };

    // Grab program header table, and load PT_LOAD segments into memory
    // TODO: dynamically create page table and set flags
    let pht = ehdr.get_pht(load_start);
    for phe in pht {
        if phe.p_type == PT_LOAD {
            println!("loading segment to vaddr {:x}, mem size: {:x}", phe.p_vaddr, phe.p_memsz);
            let load_bin = unsafe {
                core::slice::from_raw_parts(
                    (load_start + phe.p_offset as usize) as *const u8,
                    phe.p_filesz as usize,
                )
            };
            let load_dest = unsafe {
                core::slice::from_raw_parts_mut(
                    va2pa(phe.p_vaddr as usize) as *mut u8,
                    phe.p_memsz as usize,
                )
            };
            load_dest.fill(0);
            let load_dest = unsafe {
                core::slice::from_raw_parts_mut(
                    va2pa(phe.p_vaddr as usize) as *mut u8,
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
            let link_vaddr = find_link_vaddr(func_name).unwrap();
            let mut link_dest = unsafe {
                &mut *(va2pa(rela.r_offset as usize) as *mut usize)
            };
            *link_dest = link_vaddr;
            println!("link `{}` to {:x}", func_name, link_vaddr);
        }
    }

    // linking internal symbols
    let rela_dyne = rela_dyn_hdr.get_table::<Elf64Rela>(load_start);
    for rela in rela_dyne {
        let link_vaddr = rela.r_addend as usize;
        let mut link_dest = unsafe {
            &mut *(va2pa(rela.r_offset as usize) as *mut usize)
        };
        *link_dest = link_vaddr;
        println!(
            "ptr storage vaddr: {:x}, link vaddr: {:x}",
            rela.r_offset, link_vaddr
        );
    }

    println!("Execute app ...");
    // NOTE: APP cannot access MMIO pflash, so use a variable
    // in kernel space to pass app entry point
    let entry = ehdr.e_entry;
    println!("app entry point: {:x}", entry);
    // switch aspace from kernel to app
    unsafe { 
        let kernel_pg_table = aspace_save(APP_PT_PGD.as_ptr() as usize);
        core::arch::asm!("
            auipc   s0, 0x0
            addi    s0, s0, 12
            jalr    t0",
            in("t0") entry,
        );
        aspave_restore(kernel_pg_table);
    }
    load_size
}

static mut FUNC_TABLE: [(&str, usize); 3] = [
    ("__libc_start_main", 0),
    ("putchar", 0),
    ("printf", 0)
];

fn init_func_table() {
    unsafe {
        FUNC_TABLE[0].1 = libc_start_main as usize;
        FUNC_TABLE[1].1 = putchar as usize;
        FUNC_TABLE[2].1 = printf as usize;
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
    init_app_page_table();

    let img_header: &ImgHeader = unsafe { &*(PLASH_START as *const ImgHeader) };
    let app_num = img_header.app_num;
    let mut load_start = PLASH_START + IMG_HEADER_SIZE + app_num * APP_HEADER_SIZE;

    println!("Load payload ...");
    println!("putchar address: {:x}", putchar as usize);

    for i in 0..app_num {
        let load_size = load_elf(i, load_start);
        load_start += load_size;
    }
}
