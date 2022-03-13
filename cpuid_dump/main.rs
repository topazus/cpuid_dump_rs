//  Copyright (c) 2021 Umio Yasuno
//  SPDX-License-Identifier: MIT

use core::arch::x86_64::{CpuidResult, __cpuid_count};

extern crate cpuid_asm;
use cpuid_asm::{cpuid, Vendor, VendorFlag, _AX};

#[path = "./const_cpuid_dump.rs"]
mod const_cpuid_dump;
pub use crate::const_cpuid_dump::*;
#[path = "./parse_mod.rs"]
mod parse_mod;
pub use crate::parse_mod::*;
#[path = "./raw_cpuid.rs"]
mod raw_cpuid;
pub use crate::raw_cpuid::*;
#[path = "./load_file.rs"]
mod load_file;
pub use crate::load_file::*;

fn cpuid_pool() -> Vec<RawCpuid> {
    let mut pool: Vec<RawCpuid> = Vec::new();

    /* Base */
    for leaf in 0x0..=0xC {
        match leaf {
            0x4 => for sub_leaf in 0..=4 {
                pool.push(RawCpuid::exe(leaf, sub_leaf));
            },
            0x7 => for sub_leaf in 0..=1 {
                pool.push(RawCpuid::exe(leaf, sub_leaf));
            },
            _ => pool.push(RawCpuid::exe(leaf, 0x0)),
        }
    }

    /* 0xD: Processor Extended State Enumeration */
    for sub_leaf in [0x0, 0x1, 0x2, 0x9, 0xB, 0xC] {
        pool.push(RawCpuid::exe(0xD, sub_leaf));
    }

    /* 0x1F: V2 Extended Topology Enumeration Leaf, Intel */
    for sub_leaf in 0..6 {
        pool.push(RawCpuid::exe(0x1F, sub_leaf));
    }

    /* Ext */
    for leaf in _AX+0x0..=_AX+0xA {
        pool.push(RawCpuid::exe(leaf, 0x0));
    }
    for leaf in _AX+0x19..=_AX+0x21 {
        /* Cache Properties, AMD, same format as Intel Leaf:0x4 */
        const LF_80_1D: u32 = _AX + 0x1D;

        match leaf {
            LF_80_1D => for sub_leaf in 0x1..=0x4 {
                pool.push(RawCpuid::exe(leaf, sub_leaf));
            },
            _ => pool.push(RawCpuid::exe(leaf, 0x0)),
        }
    }

    return pool;
}

fn parse_pool() -> Vec<u8> {
    let mut parse_pool: Vec<u8> = Vec::new();
    let cpuid_pool = cpuid_pool();
    let vendor = VendorFlag::check();
    
    for cpuid in cpuid_pool {
        if cpuid.check_result_zero() {
            continue;
        }
        parse_pool.extend(
            cpuid.parse_fmt(&vendor).into_bytes()
        );
    }

    return parse_pool;
}

fn raw_pool() -> Vec<u8> {
    let mut pool: Vec<u8> = Vec::new();
    let cpuid_pool = cpuid_pool();

    for cpuid in cpuid_pool {
        pool.extend(
            cpuid.raw_fmt().into_bytes()
        );
    }

    return pool;
}

fn dump() {
    let mut pool: Vec<u8> = Vec::new();

    pool.extend(
        format!("   (in)EAX_xECX:  {:<10} {:<10} {:<10} {:<10}\n",
            "(out)EAX", "(out)EBX", "(out)ECX", "(out)EDX").into_bytes()
    );
    pool.extend(
        format!("{}\n", "=".repeat(TOTAL_WIDTH)).into_bytes()
    );
    pool.extend(parse_pool());
    pool.extend(b"\n");

    dump_write(&pool);
}

fn raw_dump() {
    dump_write(&raw_pool());
}

use std::thread;
fn dump_all() {
    let thread_count = cpuid_asm::CpuCoreCount::get().total_thread as usize;

    println!("   (in)EAX_xECX:  {:<10} {:<10} {:<10} {:<10}\n{}",
            "(out)EAX", "(out)EBX", "(out)ECX", "(out)EDX",
            "=".repeat(80));

    for i in 0..(thread_count) {
        thread::spawn(move || {
            cpuid_asm::pin_thread!(i);

            let mut local: Vec<u8> = Vec::new();
            let id = cpuid_asm::CpuCoreCount::get().core_id;
            local.extend(
                format!("Core ID: {:>3} / Thread: {:>3}\n", id, i).into_bytes()
            );
            local.extend(parse_pool());

            dump_write(&local);
        }).join().unwrap();
    }
}

fn raw_dump_all() {
    let thread_count = cpuid_asm::CpuCoreCount::get().total_thread;

    for i in 0..(thread_count) as usize {
        thread::spawn(move || {
            cpuid_asm::pin_thread!(i);

            let mut local: Vec<u8> = Vec::new();
            local.extend(
                format!("CPU {:>3}:\n", i).into_bytes()
            );
            local.extend(raw_pool());

            dump_write(&local);
        }).join().unwrap();
    }
}

fn dump_write(pool: &[u8]) {
    use std::io::{BufWriter, Write, stdout};
    let out = stdout();
    let mut out = BufWriter::new(out.lock());

    out.write(pool).unwrap();
}

fn save_file(save_path: String, pool: &[u8]) {
    use std::fs::File;
    use std::io::Write;

    let mut f = File::create(save_path).unwrap();
    //  let pool = parse_pool();
    f.write(pool).unwrap();
}

fn only_leaf(leaf: u32, sub_leaf: u32) {
    dump_write(
        RawCpuid::exe(leaf, sub_leaf)
            .parse_fmt(&VendorFlag::all_true())
            .into_bytes()
            .as_slice()
    )
}

struct MainOpt {
    raw: bool,
    dump_all: bool,
    save: (bool, String),
    load: (bool, String),
    only_leaf: (bool, u32, u32),
}

impl MainOpt {
    fn init() -> MainOpt {
        MainOpt {
            raw: false,
            dump_all: false,
            save: (false, format!("{}.txt",
                cpuid_asm::get_trim_proc_name().replace(" ", "_")
            )),
            load: (false, "cpuid_dump.txt".to_string()),
            only_leaf: (false, 0x0, 0x0),
        }
    }
    fn parse() -> MainOpt {
        let mut opt = MainOpt::init();
        let mut skip = false;

        let args: Vec<String> = std::env::args().collect();

        for i in 1..args.len() {
            if skip {
                skip = false;
                continue;
            }

            if !args[i].starts_with("-") {
                // eprintln!("Unknown option: {}", args[i]);
                continue;
            }

            let arg = args[i].trim_start_matches("-");

            match arg {
                "a" | "all" => opt.dump_all = true,
                "r" | "raw" => opt.raw = true,
                "s" | "save" => {
                    opt.save.0 = true;
                    opt.save.1 = match args.get(i+1) {
                        Some(v) => {
                            if v.starts_with("-") {
                                skip = true;
                                continue;
                            }

                            if std::path::Path::new(v).is_dir() {
                                format!("{}{}", v, opt.save.1)
                            } else {
                                v.to_string()
                            }
                        },
                        // use default path/file name
                        // save_path: format!("{}.txt",
                        //      cpuid_asm::get_trim_proc_name().replace(" ", "_")
                        _ => continue,
                    };
                },
                "l" | "load" => {
                    opt.load.0 = true;
                    opt.load.1 = match args.get(i+1) {
                        Some(v) => {
                            if v.starts_with("-") {
                                skip = true;
                                continue;
                            }

                            v.to_string()
                        },
                        _ => {
                            eprintln!("Please load path");
                            std::process::exit(1);
                        },
                    };
                },
                "leaf" => {
                    opt.only_leaf.0 = true;
                    opt.only_leaf.1 = match args.get(i+1) {
                        Some(v) => {
                            if v.starts_with("-") {
                                eprintln!("Please the value of leaf <u32>");
                                continue;
                            }

                            if v.starts_with("0x") {
                                u32::from_str_radix(&v[2..], 16).unwrap()
                            } else {
                                v.parse::<u32>().expect("Parse error")
                            }
                        },
                        _ => continue,
                    };
                },
                "sub_leaf" => {
                    if !opt.only_leaf.0 {
                        eprintln!("Please \"--leaf <u32>\" argument");
                    }
                    opt.only_leaf.2 = match args.get(i+1) {
                        Some(v) => {
                            if v.starts_with("-") {
                                eprintln!("Please the value of sub_leaf <u32>");
                                continue;
                            }

                            if v.starts_with("0x") {
                                u32::from_str_radix(&v[2..], 16).unwrap()
                            } else {
                                v.parse::<u32>().expect("Parse error")
                            }
                        },
                        _ => continue,
                    };
                }
                _ => eprintln!("Unknown option: {}", args[i]),
            }
        }

        return opt;
    }
}

/*
TODO: load & parse,
pub enum CpuidDumpType {
    LibCpuid,
    EtallenCpuid,
    CpuidDumpRs,
    Last,
}
*/

fn main() {
    match MainOpt::parse() {
        MainOpt { only_leaf: (true, leaf, sub_leaf), .. }
            => only_leaf(leaf, sub_leaf),
        MainOpt { load: (true, load_path), .. }
            => load_file(load_path),
        MainOpt { raw: true, save: (true, save_path), .. }
            => save_file(save_path, &raw_pool()),
        MainOpt { raw: true, dump_all: true, .. }
            => raw_dump_all(),
        MainOpt { dump_all: true, .. }
            => dump_all(),
        MainOpt { raw: true, .. }
            => raw_dump(),
        MainOpt { save: (true, save_path), .. }
            => save_file(save_path, &parse_pool()),
        _ => {
            println!("CPUID Dump");
            dump();
        },
    }
}
