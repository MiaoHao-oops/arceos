#![feature(asm_const)]
#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::{println, process::exit};

const PLASH_START: usize = 0x22000000;
const RUN_START: usize = 0xffff_ffc0_8010_0000;

struct ImgHeader {
    app_num: usize,
}

struct AppHeader {
    app_size: usize,
}

const SYS_HELLO: usize = 1;
const SYS_PUTCHAR: usize = 2;
const SYS_TERMINATE: usize = 3;
// static mut ABI_TABLE: [usize; 16] = [0; 16];

// fn register_abi(num: usize, handle: usize) {
//     unsafe { ABI_TABLE[num] = handle; }
// }

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

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    let img_header_size = size_of::<ImgHeader>();
    let app_header_size = size_of::<AppHeader>();
    let img_header: &ImgHeader = unsafe { &*(PLASH_START as *const ImgHeader) };
    let app_num = img_header.app_num;
    let mut load_start = PLASH_START + img_header_size + app_num * app_header_size;

    println!("Load payload ...");

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

        load_start += load_size;
    }
}
