#![feature(asm_const)]
#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::{println, process::exit};

const PLASH_START: usize = 0x22000000;

struct ImgHeader {
    app_num: usize,
}

struct AppHeader {
    app_size: usize,
}

const SYS_HELLO: usize = 1;
const SYS_PUTCHAR: usize = 2;
const SYS_TERMINATE: usize = 3;

fn abi_hello() {
    println!("[ABI:Hello] Hello, Apps!");
}

fn abi_putchar(c: char) {
    println!("[ABI:Print] {c}");
}

fn abi_terminate(exit_code: i32) {
    println!("[ABI:Terminate] Loader exits with code {}", exit_code);
    exit(exit_code);
}

fn abi_entry(abi_num: usize, arg0: usize) {
    match abi_num {
        SYS_HELLO => abi_hello(),
        SYS_PUTCHAR => abi_putchar(arg0 as u8 as char),
        SYS_TERMINATE => abi_terminate(arg0 as i32),
        _ => panic!("unsupport abi call!"),
    }
}

//
// App address space
//

#[link_section = ".data.app_page_table"]
static mut APP_PT_SV39: [u64; 512] = [0; 512];

unsafe fn init_app_page_table() {
    // 0x8000_0000..0xc000_0000, VRWX_GAD, 1G block
    APP_PT_SV39[2] = (0x80000 << 10) | 0xef;
    // 0xffff_ffc0_8000_0000..0xffff_ffc0_c000_0000, VRWX_GAD, 1G block
    APP_PT_SV39[0x102] = (0x80000 << 10) | 0xef;

    // 0x0000_0000..0x4000_0000, VRWX_GAD, 1G block
    APP_PT_SV39[0] = (0x00000 << 10) | 0xef;
    
    // For App aspace!
    // 0x4000_0000..0x8000_0000, VRWX_GAD, 1G block
    APP_PT_SV39[1] = (0x80000 << 10) | 0xef;
}

unsafe fn switch_app_aspace() {
    use riscv::register::satp;
    let page_table_root = APP_PT_SV39.as_ptr() as usize -
    axconfig::PHYS_VIRT_OFFSET;
    satp::set(satp::Mode::Sv39, 0, page_table_root >> 12);
    riscv::asm::sfence_vma_all();
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    let img_header_size = size_of::<ImgHeader>();
    let app_header_size = size_of::<AppHeader>();
    let img_header: &ImgHeader = unsafe { &*(PLASH_START as *const ImgHeader) };
    let app_num = img_header.app_num;
    let mut load_start = PLASH_START + img_header_size + app_num * app_header_size;

    // switch aspace from kernel to app
    unsafe { init_app_page_table(); }
    unsafe { switch_app_aspace(); }

    // app running aspace
    // SBI(0x80000000) -> App <- Kernel(0x80200000)
    // 0xffff_ffc0_0000_0000
    const RUN_START: usize = 0x4010_0000;

    println!("Load payload ...");

    // FIXME: now apps uses the SAME page table 
    for i in 0..app_num {
        let app_header: &AppHeader = unsafe {
            &*((PLASH_START + img_header_size + i * app_header_size) as *const AppHeader)
        };
        let load_size = app_header.app_size;
        
        println!("app[{}] size: {}", i, load_size);
        let load_code = unsafe {
            core::slice::from_raw_parts(load_start as *const u8, load_size)
        };
        let run_code = unsafe {
            core::slice::from_raw_parts_mut(RUN_START as *mut u8, load_size)
        };
        run_code.copy_from_slice(load_code);
        println!("run code at address [{:?}]", run_code.as_ptr());

        println!("Execute app ...");
        // execute app, pass abi_entry as first parameter
        unsafe { core::arch::asm!("
            la      a0, {abi_entry}
            li      t2, {run_start}
            jalr    t2",
            run_start = const RUN_START,
            abi_entry = sym abi_entry,
        )}
        run_code.fill(0);
        load_start += load_size;
    }
}
