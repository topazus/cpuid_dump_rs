//  © 2020 Erik Rigtorp <erik@rigtorp.se>
//  SPDX-License-Identifier: MIT
//  c2c_bench.rs was ported from c2clat:
//      https://github.com/rigtorp/c2clat
//
//  Copyright (c) 2021 Umio Yasuno
//  SPDX-License-Identifier: MIT

/*
#[cfg(target_os = "linux")]
extern crate libc;
*/
#[cfg(target_os = "linux")]
use libc::{cpu_set_t, CPU_SET, CPU_ISSET, CPU_ZERO, CPU_SETSIZE, sched_setaffinity, sched_getaffinity};

#[cfg(target_os = "windows")]
use kernel32::{GetCurrentThread, SetThreadAffinityMask};

extern crate cpuid_asm;
use cpuid_asm::pin_thread;

use std::{mem, thread, time};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

fn help_txt() {
    print!("\
        Original:\n\
        \x20    c2clat 1.0.0 © 2020 Erik Rigtorp <erik@rigtorp.se>\n\
        \x20    https://github.com/rigtorp/c2clat\n\
        \n\
        usage: c2clat [-p] [-s number_of_samples]\n\
        Plot results using gnuplot:\n\
        c2clat -p | gnuplot -p\n\
    \n");
}

fn print_matrix(title: &str, result: Vec<Vec<u128>>,
                cpu_list: &Vec<usize>, ncpu: usize, opt: &Opt) {

    macro_rules! md_table {
        ($opt: expr) => { if $opt { " |" } else { "" } }
    }

    if opt.plot {
        println!("set title \"Inter-core one-way data latency between CPU cores [{}]\"", title);
        print!("\
            set x2label \"CPU\"          \n\
            set ylabel \"CPU\"           \n\
            set cblabel \"Latency (ns)\" \n\
            $data << EOD                 \n\
        ");
    } else {
        println!("\n{}[{} (ns)]",
            if opt.md { "#### " } else { "" },
            title
        );
    }

    print!(" CPU{}", md_table!(opt.md));
    for n in cpu_list {
        print!("{:>5}{}", n, md_table!(opt.md) );
    }
    println!();
    if opt.md {
        print!(" --: | ");
        for _n in 0..ncpu {
            print!(" --: | ");
        }
        println!();
    }
    for i in 0..ncpu {
        print!("{:>4}{}", i, md_table!(opt.md) );
        for j in 0..ncpu {
            print!("{:>5}{}", result[i][j], md_table!(opt.md));
        }
        println!();
    }
    
    if opt.plot {
        print!("\
            EOD\n\
            plot \
            '$data' matrix rowheaders columnheaders using 2:1:3 with image, \
            '' matrix rowheaders columnheaders using 2:1:( sprintf(\"%g\",$3) ) with labels \
        \n");
    }
    println!();
}

struct Opt {
    md: bool,
    plot: bool,
    nsamples: isize,
}

impl Opt {
    fn default() -> Opt {
        Opt {
            md: false,
            plot: false,
            nsamples: 1_000,
        }
    }
    fn parse() -> Opt {
        let mut opt = Opt::default();
        let args: Vec<String> = std::env::args().collect();

        for i in 1..args.len() {
            let trim_val = args[i].trim_start_matches("-");

            match trim_val {
                "md" => {
                    opt.md = true;
                    opt.plot = false;
                },
                "p" | "plot" => {
                    opt.md = false;
                    opt.plot = true;
                },
                "n" => {
                    let n = match args.get(i+1) {
                        Some(v) => v.parse::<isize>().expect("Please number"),
                        None => opt.nsamples,
                    };
                    opt.nsamples = n;
                },
                _ => {},
            }
        }
        return opt;
    }
}

fn main() {
    let opt = Opt::parse();
    let nsamples = opt.nsamples;

    let mut cpus: Vec<usize> = Vec::new();

    #[cfg(target_os = "linux")]
    unsafe {
        let mut set = mem::zeroed::<cpu_set_t>();
        CPU_ZERO(&mut set);

        let status = sched_getaffinity(0, mem::size_of::<cpu_set_t>(), &mut set);
        if status == -1 {
            eprintln!("sched_getaffinity failed");
            return;
        }

        for i in 0..CPU_SETSIZE as usize {
            if CPU_ISSET(i, &set) {
                cpus.push(i);
            }
        }
    }
    #[cfg(target_os = "windows")]
    for i in 0..CPU_SETSIZE as usize {
        cpus.push(i);
    }

    let ncpu: usize = cpus.len();
    
    let mut min_result: Vec<Vec<u128>> = vec![vec![0; ncpu]; ncpu];
    let mut avg_result: Vec<Vec<u128>> = vec![vec![0; ncpu]; ncpu];

    
    // TODO: align for cache line
    #[derive(Clone)]
    struct Seq {
        v: Arc<AtomicIsize>,
        _pad: Vec<Arc<AtomicIsize>>,
    }
    impl Seq {
        fn set() -> Seq {
            let line = cpuid_asm::CacheInfo::get().l1d_line as usize;

            return Seq {
                v: Arc::new(AtomicIsize::new(-1)),
                _pad: vec![Arc::new(AtomicIsize::new(-1));
                            (line / mem::size_of::<isize>()) - 1],
            }
        }
    }

    for i in 0..(ncpu) {
        let seq1 = Seq::set();
        let seq2 = Seq::set();

        for j in (i+1)..(ncpu) {

            let _seq1 = seq1.clone();
            let _seq2 = seq2.clone();

            let c = cpus[i];

            let t = thread::spawn(move || {
                pin_thread!(c);
                for _m in 0..100 {
                    for n in 0..nsamples {
                        while _seq1.v.load(Ordering::Acquire) != n {}
                        _seq2.v.store(n, Ordering::Release);
                    }
                }
            });

            let mut tmp = u128::MAX;
            let mut avg = 0u128;

            pin_thread!(cpus[j]);
            for _m in 0..100 {
                let start = time::Instant::now();

                for n in 0..nsamples {
                    seq1.v.store(n, Ordering::Release);
                    while seq2.v.load(Ordering::Acquire) != n {}
                }

                let perf = start.elapsed();
                let perf = perf.as_nanos();

                tmp = std::cmp::min(tmp, perf);
                if _m != 0 {    // pin_thread cost
                    avg += perf;
                }
            }

            t.join().unwrap();

            min_result[i][j] = tmp / nsamples as u128 / 2;
            min_result[j][i] = min_result[i][j];

            avg_result[i][j] = avg / nsamples as u128 / (100-1) / 2;
            avg_result[j][i] = avg_result[i][j];
        }
    }

    if opt.plot {
        print!("\
            reset                                       \n\
            unset key                                   \n\
            set auto noextend                           \n\
            set autoscale fix                           \n\
            set multiplot layout 2,1                    \n\
            set palette defined (0 'white', 1 'orange') \n\
        ");
    }
    
    print_matrix("min", min_result, &cpus, ncpu, &opt);
    print_matrix("avg", avg_result, &cpus, ncpu, &opt);

    if opt.plot {
        println!("unset multiplot");
    }
}

