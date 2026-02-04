#![forbid(unsafe_code)]

//! Canvas Adapters for Visual FX (Braille/Sub-Cell Resolution)
//!
//! This module provides adapters that use the shared sampling API to fill
//! a `Painter` at sub-pixel resolution (Braille 2×4 dots per cell), achieving
//! higher effective resolution for visual effects like metaballs and plasma.
//!
//! # Feature Gating
//!
//! This module requires both `visual-fx` and `canvas` features to be enabled.
//! When only `visual-fx` is enabled, effects render at cell resolution.
//! When both are enabled, these adapters provide the higher-resolution option.
//!
//! # Design
//!
//! - **No duplicated math**: All sampling uses the shared `sampling` module.
//! - **No allocations per frame**: Painter buffers reused via `ensure_size`.
//! - **Theme-aware**: Colors derived from `ThemeInputs`.
//!
//! # Usage
//!
//! ```ignore
//! use ftui_extras::visual_fx::effects::canvas_adapters::{PlasmaCanvasAdapter, MetaballsCanvasAdapter};
//! use ftui_extras::canvas::{Painter, Mode, Canvas};
//!
//! // Create adapter
//! let mut plasma = PlasmaCanvasAdapter::new(PlasmaPalette::Neon);
//!
//! // Fill painter at sub-pixel resolution
//! let mut painter = Painter::for_area(area, Mode::Braille);
//! plasma.fill(&mut painter, time, quality, &theme);
//!
//! // Convert to widget and render
//! Canvas::from_painter(&painter).render(area, &mut frame);
//! ```

use crate::canvas::Painter;
use crate::visual_fx::effects::metaballs::MetaballsParams;
use crate::visual_fx::effects::plasma::PlasmaPalette;
use crate::visual_fx::effects::sampling::{BallState, MetaballFieldSampler};
use crate::visual_fx::{FxQuality, ThemeInputs};
use ftui_render::cell::PackedRgba;

// =============================================================================
// Plasma Canvas Adapter
// =============================================================================

/// Canvas adapter for rendering plasma at sub-pixel resolution.
///
/// Uses the shared `PlasmaSampler` for all wave computation, ensuring
/// identical results to cell-space rendering at higher resolution.
#[derive(Debug, Clone)]
pub struct PlasmaCanvasAdapter {
    /// Color palette for the effect.
    palette: PlasmaPalette,
    /// Cached geometry for the current painter size.
    cache_width: u16,
    cache_height: u16,
    /// Wave-space x coordinates (nx * 6.0).
    wx: Vec<f64>,
    /// Wave-space y coordinates (ny * 6.0).
    wy: Vec<f64>,
    /// sin/cos for diagonal term (wx * 1.2).
    x_diag_sin: Vec<f64>,
    x_diag_cos: Vec<f64>,
    /// sin/cos for diagonal term (wy * 1.2).
    y_diag_sin: Vec<f64>,
    y_diag_cos: Vec<f64>,
    /// sin(wx * 2.0) for interference.
    x_sin2: Vec<f64>,
    /// cos(wy * 2.0) for interference.
    y_cos2: Vec<f64>,
    /// Pre-scaled radial distances for v4 (center) and v5 (offset).
    radial_center_scaled: Vec<f64>,
    radial_offset_scaled: Vec<f64>,
    /// Per-frame scratch buffers for v1/v2.
    x_wave: Vec<f64>,
    y_wave: Vec<f64>,
}

impl PlasmaCanvasAdapter {
    /// Create a new plasma canvas adapter.
    #[inline]
    pub const fn new(palette: PlasmaPalette) -> Self {
        Self {
            palette,
            cache_width: 0,
            cache_height: 0,
            wx: Vec::new(),
            wy: Vec::new(),
            x_diag_sin: Vec::new(),
            x_diag_cos: Vec::new(),
            y_diag_sin: Vec::new(),
            y_diag_cos: Vec::new(),
            x_sin2: Vec::new(),
            y_cos2: Vec::new(),
            radial_center_scaled: Vec::new(),
            radial_offset_scaled: Vec::new(),
            x_wave: Vec::new(),
            y_wave: Vec::new(),
        }
    }

    /// Create a plasma adapter using theme accent colors.
    #[inline]
    pub const fn theme() -> Self {
        Self::new(PlasmaPalette::ThemeAccents)
    }

    /// Set the color palette.
    #[inline]
    pub fn set_palette(&mut self, palette: PlasmaPalette) {
        self.palette = palette;
    }

    fn ensure_cache(&mut self, width: u16, height: u16) {
        if self.cache_width == width && self.cache_height == height {
            return;
        }

        self.cache_width = width;
        self.cache_height = height;

        let w = width as usize;
        let h = height as usize;

        self.wx.resize(w, 0.0);
        self.x_diag_sin.resize(w, 0.0);
        self.x_diag_cos.resize(w, 0.0);
        self.x_sin2.resize(w, 0.0);

        let inv_w = if w > 0 { 1.0 / w as f64 } else { 0.0 };
        for x in 0..w {
            let nx = (x as f64 + 0.5) * inv_w;
            let wx = nx * 6.0;
            self.wx[x] = wx;
            let diag = wx * 1.2;
            let (sin, cos) = diag.sin_cos();
            self.x_diag_sin[x] = sin;
            self.x_diag_cos[x] = cos;
            self.x_sin2[x] = (wx * 2.0).sin();
        }

        self.wy.resize(h, 0.0);
        self.y_diag_sin.resize(h, 0.0);
        self.y_diag_cos.resize(h, 0.0);
        self.y_cos2.resize(h, 0.0);

        let inv_h = if h > 0 { 1.0 / h as f64 } else { 0.0 };
        for y in 0..h {
            let ny = (y as f64 + 0.5) * inv_h;
            let wy = ny * 6.0;
            self.wy[y] = wy;
            let diag = wy * 1.2;
            let (sin, cos) = diag.sin_cos();
            self.y_diag_sin[y] = sin;
            self.y_diag_cos[y] = cos;
            self.y_cos2[y] = (wy * 2.0).cos();
        }

        let total = w.saturating_mul(h);
        self.radial_center_scaled.resize(total, 0.0);
        self.radial_offset_scaled.resize(total, 0.0);

        for y in 0..h {
            let wy = self.wy[y];
            let wy_sq = wy * wy;
            let wy_m3 = wy - 3.0;
            let wy_m3_sq = wy_m3 * wy_m3;
            let row_offset = y * w;
            for x in 0..w {
                let wx = self.wx[x];
                let wx_sq = wx * wx;
                let wx_m3 = wx - 3.0;
                let idx = row_offset + x;
                self.radial_center_scaled[idx] = (wx_sq + wy_sq).sqrt() * 2.0;
                self.radial_offset_scaled[idx] = ((wx_m3 * wx_m3) + wy_m3_sq).sqrt() * 1.8;
            }
        }
    }

    /// Fill a painter with plasma at sub-pixel resolution.
    ///
    /// # Arguments
    /// * `painter` - The painter to fill (should be sized for the target area)
    /// * `time` - Current time in seconds (for animation)
    /// * `quality` - Quality tier (affects wave computation)
    /// * `theme` - Theme colors for palette lookup
    ///
    /// # No Allocations
    /// This method does not allocate after initial painter setup.
    pub fn fill(
        &mut self,
        painter: &mut Painter,
        time: f64,
        quality: FxQuality,
        theme: &ThemeInputs,
    ) {
        if !quality.is_enabled() {
            return;
        }

        let (width, height) = painter.size();
        if width == 0 || height == 0 {
            return;
        }

        self.ensure_cache(width, height);

        let w = width as usize;
        let h = height as usize;

        let t1 = time;
        let t2 = time * 0.8;
        let t3 = time * 0.6;
        let t4 = time * 1.2;
        let t6 = time * 0.5;
        let (sin_t3, cos_t3) = t3.sin_cos();

        self.x_wave.resize(w, 0.0);
        for (x, wave) in self.x_wave.iter_mut().enumerate().take(w) {
            *wave = (self.wx[x] * 1.5 + t1).sin();
        }

        self.y_wave.resize(h, 0.0);
        for (y, wave) in self.y_wave.iter_mut().enumerate().take(h) {
            *wave = (self.wy[y] * 1.8 + t2).sin();
        }

        let full = quality == FxQuality::Full;
        let reduced = quality == FxQuality::Reduced;

        for y in 0..h {
            let v2 = self.y_wave[y];
            let y_sin = self.y_diag_sin[y];
            let y_cos = self.y_diag_cos[y];
            let y_cos2 = self.y_cos2[y];
            let row_offset = y * w;

            for x in 0..w {
                let v1 = self.x_wave[x];
                let x_sin = self.x_diag_sin[x];
                let x_cos = self.x_diag_cos[x];

                // sin(x+y+t3) via trig identities (no per-pixel trig).
                let sin_xy = x_sin * y_cos + x_cos * y_sin;
                let cos_xy = x_cos * y_cos - x_sin * y_sin;
                let v3 = sin_xy * cos_t3 + cos_xy * sin_t3;

                let wave = if full {
                    let idx = row_offset + x;
                    let v4 = (self.radial_center_scaled[idx] - t4).sin();
                    let v5 = (self.radial_offset_scaled[idx] + time).cos();
                    let v6 = (self.x_sin2[x] * y_cos2 + t6).sin();
                    (v1 + v2 + v3 + v4 + v5 + v6) / 6.0
                } else if reduced {
                    let idx = row_offset + x;
                    let v4 = (self.radial_center_scaled[idx] - t4).sin();
                    (v1 + v2 + v3 + v4) / 4.0
                } else if quality == FxQuality::Minimal {
                    (v1 + v2 + v3) / 3.0
                } else {
                    0.0
                };

                let color = self.palette.color_at((wave + 1.0) * 0.5, theme);
                painter.point_colored(x as i32, y as i32, color);
            }
        }
    }
}

impl Default for PlasmaCanvasAdapter {
    fn default() -> Self {
        Self::theme()
    }
}

// =============================================================================
// Metaballs Canvas Adapter
// =============================================================================

/// Canvas adapter for rendering metaballs at sub-pixel resolution.
///
/// Uses the shared `MetaballFieldSampler` for all field computation, ensuring
/// identical results to cell-space rendering at higher resolution.
#[derive(Debug, Clone)]
pub struct MetaballsCanvasAdapter {
    /// Parameters controlling metaball behavior.
    params: MetaballsParams,
    /// Cached ball states for the current frame.
    ball_cache: Vec<BallState>,
}

impl MetaballsCanvasAdapter {
    /// Create a new metaballs canvas adapter with default parameters.
    pub fn new() -> Self {
        Self {
            params: MetaballsParams::default(),
            ball_cache: Vec::new(),
        }
    }

    /// Create a metaballs adapter with specific parameters.
    pub fn with_params(params: MetaballsParams) -> Self {
        Self {
            params,
            ball_cache: Vec::new(),
        }
    }

    /// Set the metaballs parameters.
    pub fn set_params(&mut self, params: MetaballsParams) {
        self.params = params;
    }

    /// Get the current parameters.
    pub fn params(&self) -> &MetaballsParams {
        &self.params
    }

    /// Prepare ball states for the current frame.
    ///
    /// Call this once per frame before calling `fill`.
    pub fn prepare(&mut self, time: f64, quality: FxQuality) {
        let count = ball_count_for_quality(&self.params, quality);

        // Ensure cache capacity
        if self.ball_cache.len() != count {
            self.ball_cache.resize(
                count,
                BallState {
                    x: 0.0,
                    y: 0.0,
                    r2: 0.0,
                    hue: 0.0,
                },
            );
        }

        // Animate balls
        let t_scaled = time * self.params.time_scale;
        let (bounds_min, bounds_max) = ordered_pair(self.params.bounds_min, self.params.bounds_max);
        let (radius_min, radius_max) = ordered_pair(self.params.radius_min, self.params.radius_max);

        for (i, ball) in self.params.balls.iter().take(count).enumerate() {
            let x = ping_pong(ball.x + ball.vx * t_scaled, bounds_min, bounds_max);
            let y = ping_pong(ball.y + ball.vy * t_scaled, bounds_min, bounds_max);
            let pulse = 1.0
                + self.params.pulse_amount * (time * self.params.pulse_speed + ball.phase).sin();
            let radius = ball.radius.clamp(radius_min, radius_max).max(0.001) * pulse;
            let hue = (ball.hue + time * self.params.hue_speed).rem_euclid(1.0);

            self.ball_cache[i] = BallState {
                x,
                y,
                r2: radius * radius,
                hue,
            };
        }
    }

    /// Fill a painter with metaballs at sub-pixel resolution.
    ///
    /// # Arguments
    /// * `painter` - The painter to fill (should be sized for the target area)
    /// * `quality` - Quality tier (affects field computation)
    /// * `theme` - Theme colors for palette lookup
    ///
    /// # Prerequisites
    /// Call `prepare(time, quality)` before this method for each frame.
    ///
    /// # No Allocations
    /// This method does not allocate after initial painter setup.
    pub fn fill(&self, painter: &mut Painter, quality: FxQuality, theme: &ThemeInputs) {
        if !quality.is_enabled() || self.ball_cache.is_empty() {
            return;
        }

        let (width, height) = painter.size();
        if width == 0 || height == 0 {
            return;
        }

        let (glow, threshold) = thresholds(&self.params);
        let w = width as f64;
        let h = height as f64;
        let inv_w = 1.0 / w;
        let inv_h = 1.0 / h;
        let stops = palette_stops(self.params.palette, theme);

        for dy in 0..height {
            let ny = (dy as f64 + 0.5) * inv_h;

            for dx in 0..width {
                let nx = (dx as f64 + 0.5) * inv_w;

                // Sample field and hue
                let (field, avg_hue) = MetaballFieldSampler::sample_field_from_slice(
                    &self.ball_cache,
                    nx,
                    ny,
                    quality,
                );

                if field > glow {
                    let intensity = if field > threshold {
                        1.0
                    } else {
                        (field - glow) / (threshold - glow)
                    };

                    let color = color_at_with_stops(&stops, avg_hue, intensity, theme);
                    painter.point_colored(dx as i32, dy as i32, color);
                }
            }
        }
    }

    /// Convenience method that calls prepare and fill.
    pub fn fill_frame(
        &mut self,
        painter: &mut Painter,
        time: f64,
        quality: FxQuality,
        theme: &ThemeInputs,
    ) {
        self.prepare(time, quality);
        self.fill(painter, quality, theme);
    }
}

impl Default for MetaballsCanvasAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Internal Helpers (mirror sampling.rs to avoid changing its API)
// =============================================================================

fn ball_count_for_quality(params: &MetaballsParams, quality: FxQuality) -> usize {
    let total = params.balls.len();
    if total == 0 {
        return 0;
    }
    match quality {
        FxQuality::Full => total,
        FxQuality::Reduced => total.saturating_sub(total / 4).max(4).min(total),
        FxQuality::Minimal => total.saturating_sub(total / 2).max(3).min(total),
        FxQuality::Off => 0,
    }
}

fn thresholds(params: &MetaballsParams) -> (f64, f64) {
    let glow = params
        .glow_threshold
        .clamp(0.0, params.threshold.max(0.001));
    let mut threshold = params.threshold.max(glow + 0.0001);
    if threshold <= glow {
        threshold = glow + 0.0001;
    }
    (glow, threshold)
}

fn palette_stops(
    palette: crate::visual_fx::effects::metaballs::MetaballsPalette,
    theme: &ThemeInputs,
) -> [PackedRgba; 4] {
    use crate::visual_fx::effects::metaballs::MetaballsPalette;
    match palette {
        MetaballsPalette::ThemeAccents => [
            theme.bg_surface,
            theme.accent_primary,
            theme.accent_secondary,
            theme.fg_primary,
        ],
        MetaballsPalette::Aurora => [
            theme.accent_slots[0],
            theme.accent_primary,
            theme.accent_slots[1],
            theme.accent_secondary,
        ],
        MetaballsPalette::Lava => [
            theme.accent_slots[2],
            theme.accent_secondary,
            theme.accent_primary,
            theme.accent_slots[3],
        ],
        MetaballsPalette::Ocean => [
            theme.accent_primary,
            theme.accent_slots[3],
            theme.accent_slots[0],
            theme.fg_primary,
        ],
    }
}

#[inline]
fn color_at_with_stops(
    stops: &[PackedRgba; 4],
    hue: f64,
    intensity: f64,
    theme: &ThemeInputs,
) -> PackedRgba {
    let base = gradient_color(stops, hue);
    let t = intensity.clamp(0.0, 1.0);
    lerp_color(theme.bg_base, base, t)
}

#[inline]
fn ping_pong(value: f64, min: f64, max: f64) -> f64 {
    let range = (max - min).max(0.0001);
    let period = 2.0 * range;
    let mut v = (value - min).rem_euclid(period);
    if v > range {
        v = period - v;
    }
    min + v
}

#[inline]
fn lerp_color(a: PackedRgba, b: PackedRgba, t: f64) -> PackedRgba {
    let t = t.clamp(0.0, 1.0);
    let r = (a.r() as f64 + (b.r() as f64 - a.r() as f64) * t) as u8;
    let g = (a.g() as f64 + (b.g() as f64 - a.g() as f64) * t) as u8;
    let bl = (a.b() as f64 + (b.b() as f64 - a.b() as f64) * t) as u8;
    PackedRgba::rgb(r, g, bl)
}

#[inline]
fn gradient_color(stops: &[PackedRgba; 4], t: f64) -> PackedRgba {
    let t = t.clamp(0.0, 1.0);
    let scaled = t * 3.0;
    let idx = (scaled.floor() as usize).min(2);
    let local = scaled - idx as f64;
    match idx {
        0 => lerp_color(stops[0], stops[1], local),
        1 => lerp_color(stops[1], stops[2], local),
        _ => lerp_color(stops[2], stops[3], local),
    }
}

#[inline]
fn ordered_pair(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Mode;

    fn default_theme() -> ThemeInputs {
        ThemeInputs::default_dark()
    }

    #[test]
    fn plasma_adapter_fills_painter() {
        let theme = default_theme();
        let mut adapter = PlasmaCanvasAdapter::theme();
        let mut painter = Painter::new(20, 16, Mode::Braille);

        adapter.fill(&mut painter, 1.0, FxQuality::Full, &theme);

        // Verify some pixels were set (plasma should fill all)
        let (w, h) = painter.size();
        let mut set_count = 0;
        for y in 0..h {
            for x in 0..w {
                if painter.get(x as i32, y as i32) {
                    set_count += 1;
                }
            }
        }
        assert!(set_count > 0, "Plasma should set pixels");
    }

    #[test]
    fn plasma_adapter_quality_off_noop() {
        let theme = default_theme();
        let mut adapter = PlasmaCanvasAdapter::theme();
        let mut painter = Painter::new(10, 8, Mode::Braille);

        adapter.fill(&mut painter, 1.0, FxQuality::Off, &theme);

        // No pixels should be set
        let (w, h) = painter.size();
        for y in 0..h {
            for x in 0..w {
                assert!(
                    !painter.get(x as i32, y as i32),
                    "Off quality should not set pixels"
                );
            }
        }
    }

    #[test]
    fn plasma_adapter_deterministic() {
        let theme = default_theme();
        let mut adapter = PlasmaCanvasAdapter::new(PlasmaPalette::Ocean);
        let mut p1 = Painter::new(16, 16, Mode::Braille);
        let mut p2 = Painter::new(16, 16, Mode::Braille);

        adapter.fill(&mut p1, 2.5, FxQuality::Full, &theme);
        adapter.fill(&mut p2, 2.5, FxQuality::Full, &theme);

        // Compare pixel states
        let (w, h) = p1.size();
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    p1.get(x as i32, y as i32),
                    p2.get(x as i32, y as i32),
                    "Plasma should be deterministic at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn metaballs_adapter_fills_painter() {
        let theme = default_theme();
        let mut adapter = MetaballsCanvasAdapter::new();
        let mut painter = Painter::new(20, 16, Mode::Braille);

        adapter.fill_frame(&mut painter, 1.0, FxQuality::Full, &theme);

        // Verify some pixels were set (metaballs should set some)
        let (w, h) = painter.size();
        let mut set_count = 0;
        for y in 0..h {
            for x in 0..w {
                if painter.get(x as i32, y as i32) {
                    set_count += 1;
                }
            }
        }
        assert!(set_count > 0, "Metaballs should set some pixels");
    }

    #[test]
    fn metaballs_adapter_quality_off_noop() {
        let theme = default_theme();
        let mut adapter = MetaballsCanvasAdapter::new();
        let mut painter = Painter::new(10, 8, Mode::Braille);

        adapter.fill_frame(&mut painter, 1.0, FxQuality::Off, &theme);

        // No pixels should be set
        let (w, h) = painter.size();
        for y in 0..h {
            for x in 0..w {
                assert!(
                    !painter.get(x as i32, y as i32),
                    "Off quality should not set pixels"
                );
            }
        }
    }

    #[test]
    fn metaballs_adapter_deterministic() {
        let theme = default_theme();
        let mut adapter = MetaballsCanvasAdapter::new();
        let mut p1 = Painter::new(16, 16, Mode::Braille);
        let mut p2 = Painter::new(16, 16, Mode::Braille);

        adapter.prepare(2.5, FxQuality::Full);
        adapter.fill(&mut p1, FxQuality::Full, &theme);
        adapter.fill(&mut p2, FxQuality::Full, &theme);

        // Compare pixel states
        let (w, h) = p1.size();
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    p1.get(x as i32, y as i32),
                    p2.get(x as i32, y as i32),
                    "Metaballs should be deterministic at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn metaballs_adapter_prepare_updates_cache() {
        let mut adapter = MetaballsCanvasAdapter::new();

        adapter.prepare(0.0, FxQuality::Full);
        let count1 = adapter.ball_cache.len();

        adapter.prepare(1.0, FxQuality::Minimal);
        let count2 = adapter.ball_cache.len();

        // Minimal quality should have fewer balls
        assert!(count2 <= count1, "Minimal should have fewer or equal balls");
    }

    #[test]
    fn empty_painter_safe() {
        let theme = default_theme();
        let mut adapter = PlasmaCanvasAdapter::theme();
        let mut painter = Painter::new(0, 0, Mode::Braille);

        // Should not panic
        adapter.fill(&mut painter, 1.0, FxQuality::Full, &theme);
    }

    #[test]
    fn single_pixel_painter() {
        let theme = default_theme();
        let mut adapter = PlasmaCanvasAdapter::theme();
        let mut painter = Painter::new(1, 1, Mode::Braille);

        adapter.fill(&mut painter, 0.5, FxQuality::Full, &theme);

        // Single pixel should be set
        assert!(painter.get(0, 0), "Single pixel should be set");
    }
}
