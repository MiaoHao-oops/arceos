#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

use core::mem::size_of;

#[cfg(feature = "axstd")]
use axstd::println;

const PLASH_START: usize = 0x22000000;

struct ImgHeader {
    size: usize,
}

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    let apps_start = (PLASH_START + size_of::<ImgHeader>()) as *const u8;
    let img_header: &ImgHeader = unsafe { &*(PLASH_START as *const ImgHeader) };
    let apps_size = img_header.size; // Dangerous!!! We need to get accurate size of apps.

    println!("Load payload ...");

    println!("app size: {}", apps_size);
    let code = unsafe { core::slice::from_raw_parts(apps_start, apps_size) };
    println!("content: {:?}: ", code);

    println!("Load payload ok!");
}