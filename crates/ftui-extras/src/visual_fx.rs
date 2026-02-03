#![forbid(unsafe_code)]

//! Visual FX primitives (feature-gated).
//!
//! This module defines the stable core types used by higher-level visual FX:
//! - background-only "backdrop" effects
//! - optional quality tiers
//! - theme input plumbing (resolved theme colors; conversions live elsewhere)
//!
//! Design goals:
//! - **Deterministic**: given the same inputs, output should be identical.
//! - **No per-frame allocations required**: effects should reuse internal buffers.
//! - **Tiny-area safe**: width/height may be zero; must not panic.

use ftui_render::cell::PackedRgba;

/// Quality hint for FX implementations.
///
/// The mapping from runtime degradation/budgets is handled elsewhere; this enum is
/// a stable "dial" so FX code can implement graceful degradation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FxQuality {
    Low,
    Medium,
    High,
}

/// Resolved theme inputs for FX.
///
/// This is intentionally a *data-only* type. Conversions from other theme systems
/// (e.g. ftui-extras themes) are tracked separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeInputs {
    /// Background color the FX should treat as the "base".
    pub bg: PackedRgba,
    /// Default foreground (used by some effects for legibility decisions).
    pub fg: PackedRgba,
    /// Accent slots for theme-coherent colorization.
    pub accents: [PackedRgba; 12],
}

impl ThemeInputs {
    #[inline]
    pub const fn new(bg: PackedRgba, fg: PackedRgba, accents: [PackedRgba; 12]) -> Self {
        Self { bg, fg, accents }
    }
}

/// Call-site provided render context.
///
/// `BackdropFx` renders into a caller-owned `out` buffer using a row-major layout:
/// `out[(y * width + x)]` for 0 <= x < width, 0 <= y < height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxContext<'a> {
    pub width: u16,
    pub height: u16,
    pub frame: u64,
    pub time_seconds: f64,
    pub quality: FxQuality,
    pub theme: &'a ThemeInputs,
}

impl<'a> FxContext<'a> {
    #[inline]
    pub const fn len(&self) -> usize {
        self.width as usize * self.height as usize
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Background-only effect that renders into a caller-owned pixel buffer.
///
/// Invariants:
/// - Implementations must tolerate `width == 0` or `height == 0` (no panic).
/// - `out.len()` is expected to equal `ctx.width * ctx.height`. Implementations may
///   debug-assert this but should not rely on it for safety.
/// - Implementations should avoid per-frame allocations; reuse internal state.
pub trait BackdropFx {
    /// Human-readable name (used for debugging / UI).
    fn name(&self) -> &'static str;

    /// Optional resize hook so effects can (re)allocate caches deterministically.
    fn resize(&mut self, _width: u16, _height: u16) {}

    /// Render into `out` (row-major, width*height).
    fn render(&mut self, ctx: FxContext<'_>, out: &mut [PackedRgba]);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SolidBg;

    impl BackdropFx for SolidBg {
        fn name(&self) -> &'static str {
            "solid-bg"
        }

        fn render(&mut self, ctx: FxContext<'_>, out: &mut [PackedRgba]) {
            if ctx.width == 0 || ctx.height == 0 {
                return;
            }
            debug_assert_eq!(out.len(), ctx.len());
            out.fill(ctx.theme.bg);
        }
    }

    #[test]
    fn smoke_backdrop_fx_renders_without_panicking() {
        let theme = ThemeInputs::new(
            PackedRgba::BLACK,
            PackedRgba::WHITE,
            [PackedRgba::WHITE; 12],
        );
        let ctx = FxContext {
            width: 4,
            height: 3,
            frame: 0,
            time_seconds: 0.0,
            quality: FxQuality::Low,
            theme: &theme,
        };
        let mut out = vec![PackedRgba::TRANSPARENT; ctx.len()];

        let mut fx = SolidBg;
        fx.render(ctx, &mut out);

        assert!(out.iter().all(|&c| c == PackedRgba::BLACK));
    }

    #[test]
    fn tiny_area_is_safe() {
        let theme = ThemeInputs::new(
            PackedRgba::BLACK,
            PackedRgba::WHITE,
            [PackedRgba::WHITE; 12],
        );
        let mut fx = SolidBg;

        let ctx = FxContext {
            width: 0,
            height: 0,
            frame: 0,
            time_seconds: 0.0,
            quality: FxQuality::Low,
            theme: &theme,
        };
        let mut out = Vec::new();
        fx.render(ctx, &mut out);
    }
}
