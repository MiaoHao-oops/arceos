#![feature(asm_const)]
#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

extern crate axlibc;
use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::println;

const PLASH_START: usize = 0x2200_0000;

struct ImgHeader {
    app_num: usize,
}

struct AppHeader {
    app_size: usize,
}

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

unsafe fn init_app_page_table() {
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

unsafe fn aspace_save(pg_table_paddr: usize) -> usize {
    use riscv::register::satp;
    let prev_satp = satp::read().bits();
    let page_table_root = pg_table_paddr -
    axconfig::PHYS_VIRT_OFFSET;
    satp::set(satp::Mode::Sv39, 0, page_table_root >> 12);
    riscv::asm::sfence_vma_all();
    prev_satp
}

unsafe fn aspave_restore(pg_table_paddr: usize) {
    use riscv::register::satp;
    satp::write(pg_table_paddr);
    riscv::asm::sfence_vma_all();
}

const IMG_HEADER_SIZE: usize = size_of::<ImgHeader>();
const APP_HEADER_SIZE: usize = size_of::<AppHeader>();

fn libc_start_main(main: usize) {
    unsafe {
        core::arch::asm!("
            mv  ra, s0
            jr  a0",
            in("a0") main,
        );
    }
}

fn load_elf(app_num: usize, load_start: &mut usize) {
    const LOAD0_OFF: usize = 0x0;
    const LOAD0_PADDR: usize = 0x80100000;
    const LOAD0_FSIZE: usize = 0x76c;
    const LOAD0_MSIZE: usize = 0x76c;
    const LOAD1_OFF: usize = 0xdf8;
    const LOAD1_PADDR: usize = 0x80101df8;
    const LOAD1_FSIZE: usize = 0x260;
    const LOAD1_MSIZE: usize = 0x268;

    const ENTRY: usize = 0x600;

    let app_header: &AppHeader = unsafe {
        &*((PLASH_START + IMG_HEADER_SIZE + app_num * APP_HEADER_SIZE) as *const AppHeader)
    };
    let load_size = app_header.app_size;
    println!("hello app ELF size: {} bytes", app_header.app_size);
    println!("load_start: {:x}", *load_start);

    let load0_bin = unsafe {
        core::slice::from_raw_parts((*load_start + LOAD0_OFF) as *const u8, LOAD0_FSIZE)
    };
    let load0_dest = unsafe {
        core::slice::from_raw_parts_mut(LOAD0_PADDR as *mut u8, LOAD0_MSIZE)
    };
    load0_dest.fill(0);
    load0_dest.copy_from_slice(load0_bin);

    let load1_bin = unsafe {
        core::slice::from_raw_parts((*load_start + LOAD1_OFF) as *const u8, LOAD1_FSIZE)
    };
    let load1_dest = unsafe {
        core::slice::from_raw_parts_mut(LOAD1_PADDR as *mut u8, LOAD1_MSIZE)
    };
    load1_dest.fill(0);
    let load1_dest = unsafe {
        core::slice::from_raw_parts_mut(LOAD1_PADDR as *mut u8, LOAD1_FSIZE)
    };
    load1_dest.copy_from_slice(load1_bin);

    unsafe {
        core::arch::asm!("
            li  t0, 0x80102008
            la  t1, {libc_start_main}
            sd  t1, 16(t0)
            la  t1, {putchar}
            sd  t1, 24(t0)
            la  t1, {printf}
            sd  t1, 32(t0)
            la  t1, 0x6de
            sd  t1, 56(t0)",
            libc_start_main = sym libc_start_main,
            putchar = sym putchar,
            printf = sym printf,
        );
    }

    println!("Execute app ...");
    // switch aspace from kernel to app
    let kernel_pg_table = unsafe {
        aspace_save(APP_PT_PGD.as_ptr() as usize)
    };
    unsafe { 
        core::arch::asm!("
            auipc   s0, 0x0
            addi    s0, s0, 16
            la      t0, {run_start}
            jalr    t0",
            run_start = const ENTRY,
        );
    }
    unsafe {
        aspave_restore(kernel_pg_table);
    }
    *load_start += load_size;
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    unsafe { init_app_page_table(); }

    let img_header: &ImgHeader = unsafe { &*(PLASH_START as *const ImgHeader) };
    let app_num = img_header.app_num;
    let mut load_start = PLASH_START + IMG_HEADER_SIZE + app_num * APP_HEADER_SIZE;

    println!("Load payload ...");
    println!("putchar address: {:x}", putchar as usize);

    for i in 0..app_num {
        load_elf(i, &mut load_start);
    }
}
