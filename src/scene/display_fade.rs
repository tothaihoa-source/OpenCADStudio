//! CadMini Studio display fades: hatches and lines drawn fainter so the
//! other shows through (HATCHFADE, LINEFADE), as CadMini's HATCH / NÉT
//! status buttons do. A uniform only: changing them needs no retessellate.
//!
//! The two amounts travel in the frame uniform's `flat_shade` float, whose
//! bit 0 keeps the mesh flat-shade flag: bits 1..7 hold the hatch fade and
//! bits 8.. the line fade, both in percent (the shaders' `hatch_keep` and
//! `line_keep`).

use std::sync::atomic::{AtomicU32, Ordering};

static HATCH_FADE: AtomicU32 = AtomicU32::new(0);
static LINE_FADE: AtomicU32 = AtomicU32::new(0);

/// Most a fade may take away, in percent.
pub const MAX_FADE: u32 = 90;

pub fn hatch_fade() -> u32 {
    HATCH_FADE.load(Ordering::Relaxed)
}

pub fn line_fade() -> u32 {
    LINE_FADE.load(Ordering::Relaxed)
}

pub fn set_hatch_fade(percent: u32) {
    HATCH_FADE.store(percent.min(MAX_FADE), Ordering::Relaxed);
}

pub fn set_line_fade(percent: u32) {
    LINE_FADE.store(percent.min(MAX_FADE), Ordering::Relaxed);
}

/// The `flat_shade` uniform value carrying the flag and both fades.
pub fn pack(flat_shade: bool) -> f32 {
    pack_values(flat_shade, hatch_fade(), line_fade())
}

pub fn pack_values(flat_shade: bool, hatch: u32, line: u32) -> f32 {
    let bits = u32::from(flat_shade) | (hatch.min(MAX_FADE) << 1) | (line.min(MAX_FADE) << 8);
    bits as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_round_trips_through_the_shader_decoding() {
        let v = pack_values(true, 50, 75) as u32;
        assert_eq!(v & 1, 1);
        assert_eq!((v >> 1) & 127, 50);
        assert_eq!(v >> 8, 75);
        // No fade, no flag: the value the shaders saw before.
        assert_eq!(pack_values(false, 0, 0), 0.0);
        assert_eq!(pack_values(true, 0, 0), 1.0);
        assert_eq!((pack_values(false, 200, 0) as u32 >> 1) & 127, MAX_FADE);
    }
}
