use crate::{CpuidResult, CpuVendor};
use super::*;

pub trait ParseGeneric {
    fn info_00_01h(&self, vendor: &CpuVendor) -> String;
    fn monitor_mwait_00_05h(&self) -> String;
    fn feature_00_01h(&self) -> String;
    fn thermal_power_00_06h(&self) -> String;
    fn feature_00_07h_x0(&self) -> String;
    fn feature_00_07h_x1(&self) -> String;
    fn topo_ext_00_0bh(&self) -> String;
    fn xstate_00_0dh(&self, sub_leaf: u32) -> String;
    fn feature_80_01h(&self) -> String;
    fn addr_size_80_08h(&self) -> String;
    fn ftr_ext_id_80_08h_ebx(&self) -> String;
    fn cpu_name(&self) -> String;
    fn cache_prop(&self) -> String;
}

impl ParseGeneric for CpuidResult {
    fn info_00_01h(&self, vendor: &CpuVendor) -> String {
        let fms = libcpuid_dump::FamModStep::from(self);
        let info01h = libcpuid_dump::Info01h::from(self);

        let proc_info = libcpuid_dump::ProcInfo::from_fms(&fms, vendor);
        let codename = match proc_info.codename {
            libcpuid_dump::CpuCodename::Unknown(_, _, _) => "".to_string(),
            _ => {
                let step_info = match proc_info.step_info {
                    libcpuid_dump::CpuStepping::Unknown(_) => "".to_string(),
                    _ => format!(" ({})", proc_info.step_info),
                };

                format!("{LN_PAD}[Codename: {}{}]", proc_info.codename, step_info)
            },
        };
        let node = match proc_info.node {
            Some(size) => format!("{LN_PAD}[ProcessNode: {size}]"),
            None => "".to_string(),
        };
        let archname = match proc_info.archname {
            libcpuid_dump::CpuMicroArch::Unknown => "".to_string(),
            _ => format!("{LN_PAD}[Arch: {}]", proc_info.archname),
        };

        [
            format!(
                "[F: {:#X}, M: {:#X}, S: {:#X}]",
                fms.syn_fam,
                fms.syn_mod,
                fms.step
            ),
            codename,
            node,
            archname,
            format!(
                "{LN_PAD}[APIC ID: {:>3}, Max: {:>3}]{LN_PAD}[CLFlush: {:3}B]",
                info01h.local_apic_id,
                info01h.max_apic_id,
                info01h.clflush_size,
            ),
        ].concat()
    }

    fn monitor_mwait_00_05h(&self) -> String {
        let min_mon_line_size = self.eax & 0xFFFF;
        let max_mon_line_size = self.ebx & 0xFFFF;
        let ftr = [
            if (self.ecx & 0b01) == 0b01 { "[EMX] " } else { "" },
            if (self.ecx & 0b10) == 0b10 { "[IBE] " } else { "" },
        ].concat();

        let c_state: String = {
            let mut c = 0;

            [
                (self.edx) & 0xF,
                (self.edx >>  4) & 0xF,
                (self.edx >>  8) & 0xF,
                (self.edx >> 12) & 0xF,
                (self.edx >> 16) & 0xF,
                (self.edx >> 20) & 0xF,
                (self.edx >> 24) & 0xF,
                (self.edx >> 28) & 0xF,
            ].map(|v| {
                let parsed = if v != 0 {
                    format!("{LN_PAD}[C{c} sub-state using MWAIT: {v}]")
                } else {
                    "".to_string()
                };

                c += 1;

                parsed
            }).concat()
        };
        
        [
            format!("[MonitorLineSize: {min_mon_line_size}(Min), {max_mon_line_size}(Max)]"),
            lnpad!(),
            ftr,
            c_state,
        ].concat()
    }

    fn feature_00_01h(&self) -> String {
        align_mold_ftr(&[
            str_detect_ftr(self.edx, &ftr_00_01_edx_x0()),
            str_detect_ftr(self.ecx, &ftr_00_01_ecx_x0()),
        ].concat())
    }

    fn thermal_power_00_06h(&self) -> String {
        align_mold_ftr(&str_detect_ftr(self.eax, &ftr_00_06_eax_x0()))
    }

    fn feature_00_07h_x0(&self) -> String {
        align_mold_ftr(&[
            str_detect_ftr(self.ebx, &ftr_00_07_ebx_x0()),
            str_detect_ftr(self.ecx, &ftr_00_07_ecx_x0()),
            str_detect_ftr(self.edx, &ftr_00_07_edx_x0()),
        ].concat())
    }

    fn feature_00_07h_x1(&self) -> String {
        align_mold_ftr(&[
            str_detect_ftr(self.eax, &ftr_00_07_eax_x1()),
            str_detect_ftr(self.edx, &ftr_00_07_edx_x1()),
        ].concat())
    }

    fn topo_ext_00_0bh(&self) -> String {
        let topo = libcpuid_dump::IntelExtTopo::from(self);

        format!("[LevelType: {}, num: {}]", topo.level_type, topo.num_proc)
    }

    fn xstate_00_0dh(&self, sub_leaf: u32) -> String {
        let size = |eax: u32, txt: &str| -> String {
            /* 00_0D_X{SUB}:EAX is the state size, EAX = 0 indicates not supported it */
            if eax != 0x0 {
                format!("[{txt:<16} save size: {eax:>3}B]")
            } else {
                "".to_string()
            }
        };

        let eax = self.eax;

        match sub_leaf {
            0x0 => {
                [
                    format!("[-XFEATURE Mask-]{LN_PAD}"),
                    align_mold_ftr(&str_detect_ftr(eax, &xfeature_mask_00_0d_eax_x0())),
                ]
                .concat()
            },
            0x1 => {
                [
                    align_mold_ftr(&str_detect_ftr(self.eax, &xsave_00_0d_eax_x1())),
                    align_mold_ftr(&str_detect_ftr(self.ecx, &xsave_00_0d_ecx_x1())),
                ]
                .concat()
            },
            0x2 => size(eax, "YMMHI"),
            0x5 => size(eax, "KREGS"),
            0x6 => size(eax, "ZMMHI"),
            0x7 => size(eax, "HIZMM"),
            0x9 => size(eax, "Protection Key"),
            0xB => size(eax, "CET User"),
            0xC => size(eax, "CET SuperVisor"),
            _ => size(eax, "Unknown"),
        }
    }

    fn feature_80_01h(&self) -> String {
        /* 0x8000_0001_E{CD}X_x0 */
        let buff = [
            str_detect_ftr(self.ecx, &ftr_80_01_ecx_x0()),
            str_detect_ftr(self.edx, &ftr_80_01_edx_x0()),
        ].concat();

        align_mold_ftr(&buff)
    }

    fn addr_size_80_08h(&self) -> String {
        const LEN: usize = "[Address size:".len();
        const PAD: &str = unsafe { std::str::from_utf8_unchecked(&[b' '; LEN]) };

        let addr_size = libcpuid_dump::AddressSize::from(self);
        let phy = addr_size.physical;
        let virt = addr_size.virtual_;

        format!("[Address size: {phy:2}-bits physical {LN_PAD}{PAD} {virt:2}-bits virtual]")
    }

    fn ftr_ext_id_80_08h_ebx(&self) -> String {
        align_mold_ftr(&str_detect_ftr(self.ebx, &ftr_80_08_ebx_x0()))
    }

    fn cpu_name(&self) -> String {
        let name = libcpuid_dump::ProcName::dec_cpuid(self);

        String::from_utf8(name).unwrap()
    }

    fn cache_prop(&self) -> String {
        let cache = match libcpuid_dump::CacheProp::option_from_cpuid(self) {
            Some(prop) => prop,
            None => return "".to_string(),
        };
        
        let inclusive = if cache.inclusive {
            " [Inclusive]"
        } else {
            ""
        }.to_string();

        [
            format!("[L{}{},{:>3}_way,{:>4}_{}]",
                cache.level,
                &cache.cache_type.to_string()[..1],
                cache.way,
                cache.size_in_the_unit(),
                &cache.size_unit.to_string()[..1],
            ),
            // format!("[Shared {}T]", cache.share_thread),
            inclusive,
        ].concat()
    }
}
