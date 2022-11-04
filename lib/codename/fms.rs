use crate::{cpuid, CpuidResult, CpuVendor};

pub(crate) enum ProcessNode {
    _UM(u8),
    NM(u8),
    Intel(u8),
}

use std::fmt;
impl fmt::Display for ProcessNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::_UM(size) => write!(f, "{size} um"),
            Self::NM(size) => write!(f, "{size} nm"),
            Self::Intel(size) => write!(f, "Intel {size}"),
        }
    }
}

impl From<ProcessNode> for String {
    fn from(s: ProcessNode) -> Self {
        s.to_string()
    }
}

pub struct ProcInfo {
    pub codename: String,
    pub archname: String,
    pub process: String,
}

impl ProcInfo {
    pub fn from_fms(fms: &FamModStep, vendor: &CpuVendor) -> Option<Self> {
        let [f, m, s] = [fms.syn_fam, fms.syn_mod, fms.step];

        match *vendor {
            CpuVendor::AuthenticAMD => match f {
                0x10 => Self::amd_fam10h(m, s),
                0x11 => Self::amd_fam11h(m, s),
                0x12 => Self::amd_fam12h(m, s),
                0x14 => Self::amd_fam14h(m, s),
                0x15 => Self::amd_fam15h(m, s),
                0x16 => Self::amd_fam16h(m, s),
                0x17 => Self::amd_fam17h(m, s),
                0x19 => Self::amd_fam19h(m, s),
                _ => None, 
            },
            CpuVendor::GenuineIntel => match f {
                0x5 => Self::intel_fam05h(m, s),
                0x6 => Self::intel_fam06h(m, s),
                _ => None,
            },
            CpuVendor::CentaurHauls |
            CpuVendor::Shanghai => match f {
                0x6 => Self::zhaoxin_fam06h(m, s),
                0x7 => Self::zhaoxin_fam07h(m, s),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn info<T: Into<String>, U: Into<String>>(code: &str, arch: T, process: U) -> Self {
        Self {
            codename: code.into(),
            archname: arch.into(),
            process: process.into(),
        }
    }
}

pub struct FamModStep {
    pub syn_fam: u32,
    pub syn_mod: u32,
    pub step: u32,
    pub raw_eax: u32,
}

impl From<u32> for FamModStep {
    fn from(eax: u32) -> Self {
        Self {
            syn_fam: ((eax >> 8) & 0xF) + ((eax >> 20) & 0xFF),
            syn_mod: ((eax >> 4) & 0xF) + ((eax >> 12) & 0xF0),
            step: eax & 0xF,
            raw_eax: eax,
        }
    }
}

impl From<&CpuidResult> for FamModStep {
    fn from(cpuid: &CpuidResult) -> Self {
        Self::from(cpuid.eax)
    }
}

impl FamModStep {
    pub fn get() -> Self {
        Self::from(&cpuid!(0x1))
    }
}
