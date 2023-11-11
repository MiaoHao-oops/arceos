#![no_std]

#[macro_use]
#[allow(unused_imports)]
extern crate axlog;
extern crate alloc;

use alloc::{vec::Vec, borrow::ToOwned, string::String};
use hermit_dtb::Dtb;

pub struct DtbInfo {
    pub memory_regions: Vec<(usize, usize)>,
    pub mmio_regions: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub enum ParseDtbErr {
    NotDtb,
}

fn parse_reg(reg: &[u8]) -> (usize, usize) {
    assert!(reg.len() == 16);
    let high: [u8; 8] = reg[8..].to_owned().try_into().unwrap();
    let low: [u8; 8] = reg[..8].to_owned().try_into().unwrap();
    (usize::from_be_bytes(low), usize::from_be_bytes(high))
}

fn region_filter(fdt: &Dtb, root: &str, filter: impl FnMut(&&str) -> bool) -> Vec<(usize, usize)> {
    fdt.enum_subnodes(root)
    .filter(filter)
    .map(|name| {
        let path = root.to_owned() + name;
        if let Some(val) = fdt.get_property(&path, "reg") {
            parse_reg(val)
        } else {
            (0, 0)
        }
    }).filter(|region| {
        region.0 != 0 && region.1 != 0
    }).collect()
}

pub fn pares_dtb(dtb_pa: usize) -> Result<DtbInfo, ParseDtbErr> {
    // Gets whole dtb and checks magic
    let fdt = unsafe {
        match Dtb::from_raw(dtb_pa as *const u8) {
            Some(dtb) => dtb,
            None => return Err(ParseDtbErr::NotDtb),
        }
    };

    // Collects memory regions from "/"
    let memory_regions = region_filter(
        &fdt,
        "/",
        |&name| {
            let path = "/".to_owned() + name;
            if let Some(_type) = fdt.get_property(&path, "device_type") {
                String::from_utf8_lossy(_type).contains("memory")
            } else {
                false
            }
        }
    );

    // Collects mmio regions from "/soc", namely "virtio_mmio"
    let mmio_regions = region_filter(
        &fdt, 
        "/soc",
        |&name| {
            name.contains("mmio")
        }
    );

    Ok(DtbInfo { memory_regions, mmio_regions })
}