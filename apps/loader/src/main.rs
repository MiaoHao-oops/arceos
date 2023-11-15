#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::println;

const PLASH_START: usize = 0x22000000;

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
    let mut apps_start = PLASH_START + img_header_size + app_num * app_header_size;

    println!("Load payload ...");

    for i in 0..app_num {
        let app_header: &AppHeader = unsafe {
            &*((PLASH_START + img_header_size + i * app_header_size) as *const AppHeader)
        };
        let app_size = app_header.app_size;
        
        println!("app[{}] size: {}", i, app_size);
        let code = unsafe { 
            core::slice::from_raw_parts(apps_start as *const u8, app_size) 
        };
        println!("content: {:?}", code);
        apps_start += app_size;
    }

    println!("Load payload ok!");
}