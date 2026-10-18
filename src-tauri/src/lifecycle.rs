use crate::domain::*;

#[derive(Default)]
pub struct MemoryWatch {
    low_samples: u32,
}
impl MemoryWatch {
    pub fn observe(&mut self, available: f64) -> bool {
        self.low_samples = if available < 1.5 * 1073741824.0 {
            self.low_samples + 1
        } else {
            0
        };
        self.low_samples >= 8
    }
}

pub fn memory_info() -> Option<MemoryInfo> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe {
        GlobalMemoryStatusEx(&mut status).ok()?;
    }
    Some(MemoryInfo {
        total_bytes: status.ullTotalPhys as f64,
        available_bytes: status.ullAvailPhys as f64,
    })
}
pub fn preflight(
    health: &RuntimeInfo,
    model: &ModelInfo,
    context: u32,
    strong: bool,
) -> Result<()> {
    let gib = 1073741824.0;
    let available = health.memory.as_ref().map(|m| m.available_bytes);
    let gpu = health.gpu.as_ref().map_or(0.0, |g| {
        (g.free_bytes - crate::models::GPU_RESERVE).max(0.0)
    });
    // Estimate from all resident weights, not the active MoE parameters. Keep
    // reserves for Windows, application use and context before allocating.
    let context_overhead = (context as f64 / 8192.0).max(1.0) * 0.5 * gib;
    let needed = (model.size - gpu * 0.8).max(0.5 * gib) + context_overhead + 4.0 * gib;
    if strong && available.is_none() {
        return Err(AppError::new("MODEL_MEMORY","Available system memory could not be checked. Continue with Balanced or prepare a Codex handoff."));
    }
    if available.is_some_and(|free| free < needed) {
        return Err(AppError::new("MODEL_MEMORY",if strong {"Strong mode needs more available memory on this PC. Close memory-heavy apps, reduce Strong context, continue with Balanced, or prepare a Codex handoff."}else{"More available memory is needed. Close memory-heavy apps or reduce this tier's context."}).detail(format!("Estimated available RAM needed: {:.1} GiB; available: {:.1} GiB. This is an estimate, not a guaranteed peak.",needed/gib,available.unwrap_or(0.0)/gib)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sustained_pressure_stops_but_brief_dip_recovers() {
        let mut w = MemoryWatch::default();
        for _ in 0..7 {
            assert!(!w.observe(1e9));
        }
        assert!(!w.observe(8e9));
        for _ in 0..7 {
            assert!(!w.observe(1e9));
        }
        assert!(w.observe(1e9));
    }
    #[test]
    fn strong_accepts_partial_placement_but_refuses_pressure() {
        let mut h = RuntimeInfo {
            status: "ready".into(),
            version: None,
            installed: true,
            models: vec![],
            message: String::new(),
            gpu: Some(GpuInfo {
                name: "GPU".into(),
                total_bytes: 12e9,
                free_bytes: 11e9,
            }),
            calibrations: vec![],
            memory: Some(MemoryInfo {
                total_bytes: 32e9,
                available_bytes: 24e9,
            }),
        };
        let m = ModelInfo {
            name: "test".into(),
            digest: String::new(),
            size: 24e9,
            parameter_size: String::new(),
            quantization: String::new(),
            loaded: false,
            vram: None,
            loaded_bytes: None,
            capability: "verified".into(),
            local: true,
            vision: true,
            thinking: true,
        };
        assert!(preflight(&h, &m, 32768, true).is_ok());
        h.memory.as_mut().unwrap().available_bytes = 8e9;
        assert_eq!(
            preflight(&h, &m, 32768, true).unwrap_err().code,
            "MODEL_MEMORY"
        );
        h.memory = None;
        assert!(preflight(&h, &m, 32768, true).is_err());
    }
}
