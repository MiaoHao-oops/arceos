#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![feature(asm_const)]

use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::println;

const PLASH_START: usize = 0x22000000;
const RUN_START: usize = 0xffff_ffc0_8010_0000;

struct ImgHeader {
    app_num: usize,
}

struct AppHeader {
    app_size: usize,
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
        println!("run code {:?}; address [{:?}]", run_code, run_code.as_ptr());

        // FIXME: Do a code instrumentation, may be unnecessary
        let ret_inst = unsafe {
            core::slice::from_raw_parts_mut((RUN_START + load_size) as *mut u8, 4)
        };
        let ret: [u8; 4] = [0x6f, 0x00, 0x00, 0x00];
        ret_inst.copy_from_slice(&ret);

        println!("Execute app ...");
        // execute app
        unsafe { core::arch::asm!("
            li      t2, {run_start}
            jalr    t2",
            run_start = const RUN_START,
        )}

        load_start += load_size;
    }

    println!("Load payload ok!");
}