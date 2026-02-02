#![forbid(unsafe_code)]

//! Animated text effects for terminal UI.
//!
//! This module provides a rich set of text animation and styling effects:
//!
//! - **Fade effects**: Smooth fade-in, fade-out, pulse
//! - **Gradient fills**: Horizontal, vertical, diagonal, radial gradients
//! - **Animated gradients**: Moving gradient patterns
//! - **Color cycling**: Rainbow, breathing, wave effects
//! - **Style animations**: Blinking, bold/dim toggle, underline wave
//! - **Character effects**: Typing, scramble, glitch
//! - **Transition overlays**: Full-screen announcement effects
//!
//! # Example
//!
//! ```rust,ignore
//! use ftui_extras::text_effects::{StyledText, TextEffect, TransitionOverlay};
//!
//! // Rainbow gradient text
//! let rainbow = StyledText::new("Hello World")
//!     .effect(TextEffect::RainbowGradient { speed: 0.1 })
//!     .time(current_time);
//!
//! // Fade-in text
//! let fading = StyledText::new("Appearing...")
//!     .effect(TextEffect::FadeIn { progress: 0.5 });
//!
//! // Pulsing glow
//! let pulse = StyledText::new("IMPORTANT")
//!     .effect(TextEffect::Pulse { speed: 2.0, min_alpha: 0.3 })
//!     .base_color(PackedRgba::rgb(255, 100, 100))
//!     .time(current_time);
//! ```

use std::f64::consts::{PI, TAU};

use ftui_core::geometry::Rect;
use ftui_render::cell::{CellAttrs, CellContent, PackedRgba, StyleFlags as CellStyleFlags};
use ftui_render::frame::Frame;
use ftui_widgets::Widget;

// =============================================================================
// Color Utilities
// =============================================================================

/// Interpolate between two colors.
pub fn lerp_color(a: PackedRgba, b: PackedRgba, t: f64) -> PackedRgba {
    let t = t.clamp(0.0, 1.0);
    let r = (a.r() as f64 + (b.r() as f64 - a.r() as f64) * t) as u8;
    let g = (a.g() as f64 + (b.g() as f64 - a.g() as f64) * t) as u8;
    let b_val = (a.b() as f64 + (b.b() as f64 - a.b() as f64) * t) as u8;
    PackedRgba::rgb(r, g, b_val)
}

/// Apply alpha/brightness to a color.
pub fn apply_alpha(color: PackedRgba, alpha: f64) -> PackedRgba {
    let alpha = alpha.clamp(0.0, 1.0);
    PackedRgba::rgb(
        (color.r() as f64 * alpha) as u8,
        (color.g() as f64 * alpha) as u8,
        (color.b() as f64 * alpha) as u8,
    )
}

/// Convert HSV to RGB.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> PackedRgba {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    PackedRgba::rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Multi-stop color gradient.
#[derive(Debug, Clone)]
pub struct ColorGradient {
    stops: Vec<(f64, PackedRgba)>,
}

impl ColorGradient {
    /// Create a new gradient with color stops.
    /// Stops should be tuples of (position, color) where position is 0.0 to 1.0.
    pub fn new(stops: Vec<(f64, PackedRgba)>) -> Self {
        let mut stops = stops;
        stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Self { stops }
    }

    /// Create a rainbow gradient.
    pub fn rainbow() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(255, 0, 0)),    // Red
            (0.17, PackedRgba::rgb(255, 127, 0)), // Orange
            (0.33, PackedRgba::rgb(255, 255, 0)), // Yellow
            (0.5, PackedRgba::rgb(0, 255, 0)),    // Green
            (0.67, PackedRgba::rgb(0, 127, 255)), // Blue
            (0.83, PackedRgba::rgb(127, 0, 255)), // Indigo
            (1.0, PackedRgba::rgb(255, 0, 255)),  // Violet
        ])
    }

    /// Create a sunset gradient (purple -> pink -> orange -> yellow).
    pub fn sunset() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(80, 20, 120)),
            (0.33, PackedRgba::rgb(255, 50, 120)),
            (0.66, PackedRgba::rgb(255, 150, 50)),
            (1.0, PackedRgba::rgb(255, 255, 150)),
        ])
    }

    /// Create an ocean gradient (deep blue -> cyan -> seafoam).
    pub fn ocean() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(10, 30, 100)),
            (0.5, PackedRgba::rgb(30, 180, 220)),
            (1.0, PackedRgba::rgb(150, 255, 200)),
        ])
    }

    /// Create a cyberpunk gradient (hot pink -> purple -> cyan).
    pub fn cyberpunk() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(255, 20, 150)),
            (0.5, PackedRgba::rgb(150, 50, 200)),
            (1.0, PackedRgba::rgb(50, 220, 255)),
        ])
    }

    /// Create a fire gradient (black -> red -> orange -> yellow -> white).
    pub fn fire() -> Self {
        Self::new(vec![
            (0.0, PackedRgba::rgb(0, 0, 0)),
            (0.2, PackedRgba::rgb(80, 10, 0)),
            (0.4, PackedRgba::rgb(200, 50, 0)),
            (0.6, PackedRgba::rgb(255, 150, 20)),
            (0.8, PackedRgba::rgb(255, 230, 100)),
            (1.0, PackedRgba::rgb(255, 255, 220)),
        ])
    }

    /// Sample the gradient at position t (0.0 to 1.0).
    pub fn sample(&self, t: f64) -> PackedRgba {
        let t = t.clamp(0.0, 1.0);

        if self.stops.is_empty() {
            return PackedRgba::rgb(255, 255, 255);
        }
        if self.stops.len() == 1 {
            return self.stops[0].1;
        }

        // Find the two stops we're between
        let mut prev = &self.stops[0];
        for stop in &self.stops {
            if stop.0 >= t {
                if stop.0 == prev.0 {
                    return stop.1;
                }
                let local_t = (t - prev.0) / (stop.0 - prev.0);
                return lerp_color(prev.1, stop.1, local_t);
            }
            prev = stop;
        }

        self.stops
            .last()
            .map(|s| s.1)
            .unwrap_or(PackedRgba::rgb(255, 255, 255))
    }
}

// =============================================================================
// Text Effects
// =============================================================================

/// Available text effects.
#[derive(Debug, Clone, Default)]
pub enum TextEffect {
    /// No effect, plain text.
    #[default]
    None,

    // --- Fade Effects ---
    /// Fade in from transparent to opaque.
    FadeIn {
        /// Progress from 0.0 (invisible) to 1.0 (visible).
        progress: f64,
    },
    /// Fade out from opaque to transparent.
    FadeOut {
        /// Progress from 0.0 (visible) to 1.0 (invisible).
        progress: f64,
    },
    /// Pulsing fade (breathing effect).
    Pulse {
        /// Oscillation speed (cycles per second).
        speed: f64,
        /// Minimum alpha (0.0 to 1.0).
        min_alpha: f64,
    },

    // --- Gradient Effects ---
    /// Horizontal gradient across text.
    HorizontalGradient {
        /// Gradient to use.
        gradient: ColorGradient,
    },
    /// Animated horizontal gradient.
    AnimatedGradient {
        /// Gradient to use.
        gradient: ColorGradient,
        /// Animation speed.
        speed: f64,
    },
    /// Rainbow colors cycling through text.
    RainbowGradient {
        /// Animation speed.
        speed: f64,
    },

    // --- Color Cycling ---
    /// Cycle through colors (all characters same color).
    ColorCycle {
        /// Colors to cycle through.
        colors: Vec<PackedRgba>,
        /// Cycle speed.
        speed: f64,
    },
    /// Wave effect - color moves through text like a wave.
    ColorWave {
        /// Primary color.
        color1: PackedRgba,
        /// Secondary color.
        color2: PackedRgba,
        /// Wave speed.
        speed: f64,
        /// Wave length (characters per cycle).
        wavelength: f64,
    },

    // --- Glow Effects ---
    /// Static glow around text.
    Glow {
        /// Glow color (usually a brighter version of base).
        color: PackedRgba,
        /// Intensity (0.0 to 1.0).
        intensity: f64,
    },
    /// Animated glow that pulses.
    PulsingGlow {
        /// Glow color.
        color: PackedRgba,
        /// Pulse speed.
        speed: f64,
    },

    // --- Character Effects ---
    /// Typewriter effect - characters appear one by one.
    Typewriter {
        /// Number of characters visible (can be fractional for smooth animation).
        visible_chars: f64,
    },
    /// Scramble effect - random characters that resolve to final text.
    Scramble {
        /// Progress from 0.0 (scrambled) to 1.0 (resolved).
        progress: f64,
    },
    /// Glitch effect - occasional character corruption.
    Glitch {
        /// Glitch intensity (0.0 to 1.0).
        intensity: f64,
    },
}

// =============================================================================
// StyledText - Text with effects
// =============================================================================

/// Text widget with animated effects.
#[derive(Debug, Clone)]
pub struct StyledText {
    text: String,
    effect: TextEffect,
    base_color: PackedRgba,
    bg_color: Option<PackedRgba>,
    bold: bool,
    italic: bool,
    underline: bool,
    time: f64,
    seed: u64,
}

impl StyledText {
    /// Create new styled text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            effect: TextEffect::None,
            base_color: PackedRgba::rgb(255, 255, 255),
            bg_color: None,
            bold: false,
            italic: false,
            underline: false,
            time: 0.0,
            seed: 12345,
        }
    }

    /// Set the text effect.
    pub fn effect(mut self, effect: TextEffect) -> Self {
        self.effect = effect;
        self
    }

    /// Set the base text color.
    pub fn base_color(mut self, color: PackedRgba) -> Self {
        self.base_color = color;
        self
    }

    /// Set the background color.
    pub fn bg_color(mut self, color: PackedRgba) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// Make text bold.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Make text italic.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Make text underlined.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Set the animation time (for time-based effects).
    pub fn time(mut self, time: f64) -> Self {
        self.time = time;
        self
    }

    /// Set random seed for scramble/glitch effects.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Get the length of the text.
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Check if text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Calculate the color for a character at position `idx`.
    fn char_color(&self, idx: usize, total: usize) -> PackedRgba {
        let t = if total > 1 {
            idx as f64 / (total - 1) as f64
        } else {
            0.5
        };

        match &self.effect {
            TextEffect::None => self.base_color,

            TextEffect::FadeIn { progress } => apply_alpha(self.base_color, *progress),

            TextEffect::FadeOut { progress } => apply_alpha(self.base_color, 1.0 - progress),

            TextEffect::Pulse { speed, min_alpha } => {
                let alpha =
                    min_alpha + (1.0 - min_alpha) * (0.5 + 0.5 * (self.time * speed * TAU).sin());
                apply_alpha(self.base_color, alpha)
            }

            TextEffect::HorizontalGradient { gradient } => gradient.sample(t),

            TextEffect::AnimatedGradient { gradient, speed } => {
                let animated_t = (t + self.time * speed).rem_euclid(1.0);
                gradient.sample(animated_t)
            }

            TextEffect::RainbowGradient { speed } => {
                let hue = ((t + self.time * speed) * 360.0).rem_euclid(360.0);
                hsv_to_rgb(hue, 1.0, 1.0)
            }

            TextEffect::ColorCycle { colors, speed } => {
                if colors.is_empty() {
                    return self.base_color;
                }
                let cycle_pos = (self.time * speed).rem_euclid(colors.len() as f64);
                let idx1 = cycle_pos as usize % colors.len();
                let idx2 = (idx1 + 1) % colors.len();
                let local_t = cycle_pos.fract();
                lerp_color(colors[idx1], colors[idx2], local_t)
            }

            TextEffect::ColorWave {
                color1,
                color2,
                speed,
                wavelength,
            } => {
                let phase = t * TAU * (total as f64 / wavelength) - self.time * speed;
                let wave = 0.5 + 0.5 * phase.sin();
                lerp_color(*color1, *color2, wave)
            }

            TextEffect::Glow { color, intensity } => {
                lerp_color(self.base_color, *color, *intensity)
            }

            TextEffect::PulsingGlow { color, speed } => {
                let intensity = 0.5 + 0.5 * (self.time * speed * TAU).sin();
                lerp_color(self.base_color, *color, intensity)
            }

            TextEffect::Typewriter { visible_chars } => {
                if (idx as f64) < *visible_chars {
                    self.base_color
                } else {
                    PackedRgba::TRANSPARENT
                }
            }

            TextEffect::Scramble { progress: _ } | TextEffect::Glitch { intensity: _ } => {
                self.base_color
            }
        }
    }

    /// Get the character to display at position `idx`.
    fn char_at(&self, idx: usize, original: char) -> char {
        match &self.effect {
            TextEffect::Scramble { progress } => {
                if *progress >= 1.0 {
                    return original;
                }
                // Characters resolve from left to right based on progress
                let total = self.text.chars().count();
                let resolve_threshold = idx as f64 / total as f64;
                if *progress > resolve_threshold {
                    original
                } else {
                    // Random character based on time and position
                    let hash = self
                        .seed
                        .wrapping_mul(idx as u64 + 1)
                        .wrapping_add((self.time * 10.0) as u64);
                    let ascii = 33 + (hash % 94) as u8;
                    ascii as char
                }
            }

            TextEffect::Glitch { intensity } => {
                if *intensity <= 0.0 {
                    return original;
                }
                // Random glitch based on time
                let hash = self
                    .seed
                    .wrapping_mul(idx as u64 + 1)
                    .wrapping_add((self.time * 30.0) as u64);
                let glitch_chance = (hash % 1000) as f64 / 1000.0;
                if glitch_chance < *intensity * 0.3 {
                    let ascii = 33 + (hash % 94) as u8;
                    ascii as char
                } else {
                    original
                }
            }

            TextEffect::Typewriter { visible_chars } => {
                if (idx as f64) < *visible_chars {
                    original
                } else {
                    ' '
                }
            }

            _ => original,
        }
    }

    /// Render at a specific position.
    pub fn render_at(&self, x: u16, y: u16, frame: &mut Frame) {
        let total = self.text.chars().count();
        if total == 0 {
            return;
        }

        for (i, ch) in self.text.chars().enumerate() {
            let px = x.saturating_add(i as u16);
            let color = self.char_color(i, total);
            let display_char = self.char_at(i, ch);

            // Skip fully transparent
            if color.r() == 0
                && color.g() == 0
                && color.b() == 0
                && matches!(
                    self.effect,
                    TextEffect::FadeIn { .. } | TextEffect::FadeOut { .. }
                )
            {
                continue;
            }

            if let Some(cell) = frame.buffer.get_mut(px, y) {
                cell.content = CellContent::from_char(display_char);
                cell.fg = color;

                if let Some(bg) = self.bg_color {
                    cell.bg = bg;
                }

                let mut flags = CellStyleFlags::empty();
                if self.bold {
                    flags = flags.union(CellStyleFlags::BOLD);
                }
                if self.italic {
                    flags = flags.union(CellStyleFlags::ITALIC);
                }
                if self.underline {
                    flags = flags.union(CellStyleFlags::UNDERLINE);
                }
                cell.attrs = CellAttrs::new(flags, 0);
            }
        }
    }
}

impl Widget for StyledText {
    fn render(&self, area: Rect, frame: &mut Frame) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        self.render_at(area.x, area.y, frame);
    }
}

// =============================================================================
// TransitionOverlay - Full-screen announcement effect
// =============================================================================

/// A centered overlay for displaying transition text with fade effects.
///
/// Progress goes from 0.0 (invisible) to 0.5 (peak visibility) to 1.0 (invisible).
/// This creates a smooth fade-in then fade-out animation.
#[derive(Debug, Clone)]
pub struct TransitionOverlay {
    title: String,
    subtitle: String,
    progress: f64,
    primary_color: PackedRgba,
    secondary_color: PackedRgba,
    gradient: Option<ColorGradient>,
    time: f64,
}

impl TransitionOverlay {
    /// Create a new transition overlay.
    pub fn new(title: impl Into<String>, subtitle: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: subtitle.into(),
            progress: 0.0,
            primary_color: PackedRgba::rgb(255, 100, 200),
            secondary_color: PackedRgba::rgb(180, 180, 220),
            gradient: None,
            time: 0.0,
        }
    }

    /// Set progress (0.0 = invisible, 0.5 = peak, 1.0 = invisible).
    pub fn progress(mut self, progress: f64) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    /// Set the primary (title) color.
    pub fn primary_color(mut self, color: PackedRgba) -> Self {
        self.primary_color = color;
        self
    }

    /// Set the secondary (subtitle) color.
    pub fn secondary_color(mut self, color: PackedRgba) -> Self {
        self.secondary_color = color;
        self
    }

    /// Use an animated gradient for the title.
    pub fn gradient(mut self, gradient: ColorGradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Set animation time.
    pub fn time(mut self, time: f64) -> Self {
        self.time = time;
        self
    }

    /// Calculate opacity from progress.
    fn opacity(&self) -> f64 {
        (self.progress * PI).sin()
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.opacity() > 0.01
    }
}

impl Widget for TransitionOverlay {
    fn render(&self, area: Rect, frame: &mut Frame) {
        let opacity = self.opacity();
        if opacity < 0.01 || area.width < 10 || area.height < 3 {
            return;
        }

        // Center the title
        let title_len = self.title.chars().count() as u16;
        let title_x = area.x + area.width.saturating_sub(title_len) / 2;
        let title_y = area.y + area.height / 2;

        // Render title with gradient or fade
        let title_effect = if let Some(gradient) = &self.gradient {
            TextEffect::AnimatedGradient {
                gradient: gradient.clone(),
                speed: 0.3,
            }
        } else {
            TextEffect::FadeIn { progress: opacity }
        };

        let title_text = StyledText::new(&self.title)
            .effect(title_effect)
            .base_color(apply_alpha(self.primary_color, opacity))
            .bold()
            .time(self.time);
        title_text.render_at(title_x, title_y, frame);

        // Render subtitle
        if !self.subtitle.is_empty() && title_y + 1 < area.y + area.height {
            let subtitle_len = self.subtitle.chars().count() as u16;
            let subtitle_x = area.x + area.width.saturating_sub(subtitle_len) / 2;
            let subtitle_y = title_y + 1;

            let subtitle_text = StyledText::new(&self.subtitle)
                .effect(TextEffect::FadeIn {
                    progress: opacity * 0.85,
                })
                .base_color(self.secondary_color)
                .italic()
                .time(self.time);
            subtitle_text.render_at(subtitle_x, subtitle_y, frame);
        }
    }
}

// =============================================================================
// TransitionState - Animation state manager
// =============================================================================

/// Helper for managing transition animations.
#[derive(Debug, Clone)]
pub struct TransitionState {
    progress: f64,
    active: bool,
    speed: f64,
    title: String,
    subtitle: String,
    color: PackedRgba,
    gradient: Option<ColorGradient>,
    time: f64,
}

impl Default for TransitionState {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionState {
    /// Create new transition state.
    pub fn new() -> Self {
        Self {
            progress: 0.0,
            active: false,
            speed: 0.05,
            title: String::new(),
            subtitle: String::new(),
            color: PackedRgba::rgb(255, 100, 200),
            gradient: None,
            time: 0.0,
        }
    }

    /// Start a transition.
    pub fn start(
        &mut self,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        color: PackedRgba,
    ) {
        self.title = title.into();
        self.subtitle = subtitle.into();
        self.color = color;
        self.gradient = None;
        self.progress = 0.0;
        self.active = true;
    }

    /// Start a transition with gradient.
    pub fn start_with_gradient(
        &mut self,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        gradient: ColorGradient,
    ) {
        self.title = title.into();
        self.subtitle = subtitle.into();
        self.gradient = Some(gradient);
        self.progress = 0.0;
        self.active = true;
    }

    /// Set transition speed.
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.01, 0.5);
    }

    /// Update the transition (call every tick).
    pub fn tick(&mut self) {
        self.time += 0.1;
        if self.active {
            self.progress += self.speed;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.active = false;
            }
        }
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.active || (self.progress > 0.0 && self.progress < 1.0)
    }

    /// Check if active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get current progress.
    pub fn progress(&self) -> f64 {
        self.progress
    }

    /// Get the overlay widget.
    pub fn overlay(&self) -> TransitionOverlay {
        let mut overlay = TransitionOverlay::new(&self.title, &self.subtitle)
            .progress(self.progress)
            .primary_color(self.color)
            .time(self.time);

        if let Some(ref gradient) = self.gradient {
            overlay = overlay.gradient(gradient.clone());
        }

        overlay
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lerp_color() {
        let black = PackedRgba::rgb(0, 0, 0);
        let white = PackedRgba::rgb(255, 255, 255);
        let mid = lerp_color(black, white, 0.5);
        assert_eq!(mid.r(), 127);
    }

    #[test]
    fn test_color_gradient() {
        let gradient = ColorGradient::rainbow();
        let red = gradient.sample(0.0);
        assert!(red.r() > 200);

        let mid = gradient.sample(0.5);
        assert!(mid.g() > 200); // Should be greenish
    }

    #[test]
    fn test_styled_text_effects() {
        let text = StyledText::new("Hello")
            .effect(TextEffect::RainbowGradient { speed: 1.0 })
            .time(0.5);

        assert_eq!(text.len(), 5);
        assert!(!text.is_empty());
    }

    #[test]
    fn test_transition_state() {
        let mut state = TransitionState::new();
        assert!(!state.is_active());

        state.start("Title", "Sub", PackedRgba::rgb(255, 0, 0));
        assert!(state.is_active());

        for _ in 0..50 {
            state.tick();
        }
        assert!(!state.is_active());
    }

    #[test]
    fn test_scramble_effect() {
        let text = StyledText::new("TEST")
            .effect(TextEffect::Scramble { progress: 0.0 })
            .seed(42)
            .time(1.0);

        // At progress 0, characters should be scrambled
        let ch = text.char_at(0, 'T');
        // The scrambled char will be random but not necessarily 'T'
        assert!(ch.is_ascii_graphic());
    }

    #[test]
    fn test_ascii_art_basic() {
        let art = AsciiArtText::new("HI", AsciiArtStyle::Block);
        let lines = art.render_lines();
        assert!(!lines.is_empty());
        // Block style produces 5-line characters
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_ascii_art_styles() {
        for style in [
            AsciiArtStyle::Block,
            AsciiArtStyle::Banner,
            AsciiArtStyle::Mini,
            AsciiArtStyle::Slant,
        ] {
            let art = AsciiArtText::new("A", style);
            let lines = art.render_lines();
            assert!(!lines.is_empty());
        }
    }
}

// =============================================================================
// ASCII Art Text - Figlet-style large text
// =============================================================================

/// ASCII art font styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiArtStyle {
    /// Large block letters using Unicode block characters.
    Block,
    /// Classic banner-style with slashes and pipes.
    Banner,
    /// Minimal 3-line height for compact display.
    Mini,
    /// Slanted italic-like style.
    Slant,
    /// Doom-style chunky letters.
    Doom,
    /// Small caps using Unicode characters.
    SmallCaps,
}

/// ASCII art text renderer.
#[derive(Debug, Clone)]
pub struct AsciiArtText {
    text: String,
    style: AsciiArtStyle,
    color: Option<PackedRgba>,
    gradient: Option<ColorGradient>,
}

impl AsciiArtText {
    /// Create new ASCII art text.
    pub fn new(text: impl Into<String>, style: AsciiArtStyle) -> Self {
        Self {
            text: text.into().to_uppercase(),
            style,
            color: None,
            gradient: None,
        }
    }

    /// Set text color.
    pub fn color(mut self, color: PackedRgba) -> Self {
        self.color = Some(color);
        self
    }

    /// Use a gradient for coloring.
    pub fn gradient(mut self, gradient: ColorGradient) -> Self {
        self.gradient = Some(gradient);
        self
    }

    /// Get the height in lines for this style.
    pub fn height(&self) -> usize {
        match self.style {
            AsciiArtStyle::Block => 5,
            AsciiArtStyle::Banner => 6,
            AsciiArtStyle::Mini => 3,
            AsciiArtStyle::Slant => 5,
            AsciiArtStyle::Doom => 8,
            AsciiArtStyle::SmallCaps => 1,
        }
    }

    /// Get the width for a single character.
    #[allow(dead_code)]
    fn char_width(&self) -> usize {
        match self.style {
            AsciiArtStyle::Block => 6,
            AsciiArtStyle::Banner => 6,
            AsciiArtStyle::Mini => 4,
            AsciiArtStyle::Slant => 6,
            AsciiArtStyle::Doom => 8,
            AsciiArtStyle::SmallCaps => 1,
        }
    }

    /// Render to vector of lines.
    pub fn render_lines(&self) -> Vec<String> {
        let height = self.height();
        let mut lines = vec![String::new(); height];

        for ch in self.text.chars() {
            let char_lines = self.render_char(ch);
            for (i, line) in char_lines.iter().enumerate() {
                if i < lines.len() {
                    lines[i].push_str(line);
                }
            }
        }

        lines
    }

    /// Render a single character to lines.
    fn render_char(&self, ch: char) -> Vec<&'static str> {
        match self.style {
            AsciiArtStyle::Block => self.render_block(ch),
            AsciiArtStyle::Banner => self.render_banner(ch),
            AsciiArtStyle::Mini => self.render_mini(ch),
            AsciiArtStyle::Slant => self.render_slant(ch),
            AsciiArtStyle::Doom => self.render_doom(ch),
            AsciiArtStyle::SmallCaps => self.render_small_caps(ch),
        }
    }

    fn render_block(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec!["  █   ", " █ █  ", "█████ ", "█   █ ", "█   █ "],
            'B' => vec!["████  ", "█   █ ", "████  ", "█   █ ", "████  "],
            'C' => vec![" ████ ", "█     ", "█     ", "█     ", " ████ "],
            'D' => vec!["████  ", "█   █ ", "█   █ ", "█   █ ", "████  "],
            'E' => vec!["█████ ", "█     ", "███   ", "█     ", "█████ "],
            'F' => vec!["█████ ", "█     ", "███   ", "█     ", "█     "],
            'G' => vec![" ████ ", "█     ", "█  ██ ", "█   █ ", " ████ "],
            'H' => vec!["█   █ ", "█   █ ", "█████ ", "█   █ ", "█   █ "],
            'I' => vec!["█████ ", "  █   ", "  █   ", "  █   ", "█████ "],
            'J' => vec!["█████ ", "   █  ", "   █  ", "█  █  ", " ██   "],
            'K' => vec!["█   █ ", "█  █  ", "███   ", "█  █  ", "█   █ "],
            'L' => vec!["█     ", "█     ", "█     ", "█     ", "█████ "],
            'M' => vec!["█   █ ", "██ ██ ", "█ █ █ ", "█   █ ", "█   █ "],
            'N' => vec!["█   █ ", "██  █ ", "█ █ █ ", "█  ██ ", "█   █ "],
            'O' => vec![" ███  ", "█   █ ", "█   █ ", "█   █ ", " ███  "],
            'P' => vec!["████  ", "█   █ ", "████  ", "█     ", "█     "],
            'Q' => vec![" ███  ", "█   █ ", "█   █ ", "█  █  ", " ██ █ "],
            'R' => vec!["████  ", "█   █ ", "████  ", "█  █  ", "█   █ "],
            'S' => vec![" ████ ", "█     ", " ███  ", "    █ ", "████  "],
            'T' => vec!["█████ ", "  █   ", "  █   ", "  █   ", "  █   "],
            'U' => vec!["█   █ ", "█   █ ", "█   █ ", "█   █ ", " ███  "],
            'V' => vec!["█   █ ", "█   █ ", "█   █ ", " █ █  ", "  █   "],
            'W' => vec!["█   █ ", "█   █ ", "█ █ █ ", "██ ██ ", "█   █ "],
            'X' => vec!["█   █ ", " █ █  ", "  █   ", " █ █  ", "█   █ "],
            'Y' => vec!["█   █ ", " █ █  ", "  █   ", "  █   ", "  █   "],
            'Z' => vec!["█████ ", "   █  ", "  █   ", " █    ", "█████ "],
            '0' => vec![" ███  ", "█  ██ ", "█ █ █ ", "██  █ ", " ███  "],
            '1' => vec!["  █   ", " ██   ", "  █   ", "  █   ", " ███  "],
            '2' => vec![" ███  ", "█   █ ", "  ██  ", " █    ", "█████ "],
            '3' => vec!["████  ", "    █ ", " ███  ", "    █ ", "████  "],
            '4' => vec!["█   █ ", "█   █ ", "█████ ", "    █ ", "    █ "],
            '5' => vec!["█████ ", "█     ", "████  ", "    █ ", "████  "],
            '6' => vec![" ███  ", "█     ", "████  ", "█   █ ", " ███  "],
            '7' => vec!["█████ ", "    █ ", "   █  ", "  █   ", "  █   "],
            '8' => vec![" ███  ", "█   █ ", " ███  ", "█   █ ", " ███  "],
            '9' => vec![" ███  ", "█   █ ", " ████ ", "    █ ", " ███  "],
            ' ' => vec!["      ", "      ", "      ", "      ", "      "],
            '!' => vec!["  █   ", "  █   ", "  █   ", "      ", "  █   "],
            '?' => vec![" ███  ", "█   █ ", "  ██  ", "      ", "  █   "],
            '.' => vec!["      ", "      ", "      ", "      ", "  █   "],
            '-' => vec!["      ", "      ", "█████ ", "      ", "      "],
            ':' => vec!["      ", "  █   ", "      ", "  █   ", "      "],
            _ => vec!["█████ ", "█   █ ", "█   █ ", "█   █ ", "█████ "],
        }
    }

    fn render_banner(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec![
                "  /\\  ", " /  \\ ", "/----\\", "/    \\", "/    \\", "      ",
            ],
            'B' => vec![
                "==\\   ", "| /=\\ ", "||__/ ", "| /=\\ ", "==/   ", "      ",
            ],
            'C' => vec![" /===\\", "|     ", "|     ", "|     ", " \\===/", "      "],
            'D' => vec!["==\\   ", "| \\   ", "|  |  ", "| /   ", "==/   ", "      "],
            'E' => vec!["|===| ", "|     ", "|===  ", "|     ", "|===| ", "      "],
            'F' => vec!["|===| ", "|     ", "|===  ", "|     ", "|     ", "      "],
            'G' => vec![" /===\\", "|     ", "| /==|", "|    |", " \\===/", "      "],
            'H' => vec!["|   | ", "|   | ", "|===| ", "|   | ", "|   | ", "      "],
            'I' => vec!["|===| ", "  |   ", "  |   ", "  |   ", "|===| ", "      "],
            'J' => vec!["|===| ", "   |  ", "   |  ", "|  |  ", " \\/   ", "      "],
            'K' => vec!["|  /  ", "| /   ", "|<    ", "| \\   ", "|  \\  ", "      "],
            'L' => vec!["|     ", "|     ", "|     ", "|     ", "|===| ", "      "],
            'M' => vec!["|\\  /|", "| \\/ |", "|    |", "|    |", "|    |", "      "],
            'N' => vec![
                "|\\   |", "| \\  |", "|  \\ |", "|   \\|", "|    |", "      ",
            ],
            'O' => vec![" /==\\ ", "|    |", "|    |", "|    |", " \\==/ ", "      "],
            'P' => vec!["|===\\ ", "|   | ", "|===/ ", "|     ", "|     ", "      "],
            'Q' => vec![
                " /==\\ ", "|    |", "|    |", "|  \\ |", " \\==\\/", "      ",
            ],
            'R' => vec![
                "|===\\ ", "|   | ", "|===/ ", "|  \\  ", "|   \\ ", "      ",
            ],
            'S' => vec![
                " /===\\", "|     ", " \\==\\ ", "     |", "\\===/ ", "      ",
            ],
            'T' => vec!["|===| ", "  |   ", "  |   ", "  |   ", "  |   ", "      "],
            'U' => vec!["|   | ", "|   | ", "|   | ", "|   | ", " \\=/ ", "      "],
            'V' => vec!["|   | ", "|   | ", " \\ /  ", "  |   ", "  |   ", "      "],
            'W' => vec![
                "|    |", "|    |", "| /\\ |", "|/  \\|", "/    \\", "      ",
            ],
            'X' => vec![
                "\\   / ", " \\ /  ", "  X   ", " / \\  ", "/   \\ ", "      ",
            ],
            'Y' => vec!["\\   / ", " \\ /  ", "  |   ", "  |   ", "  |   ", "      "],
            'Z' => vec!["|===| ", "   /  ", "  /   ", " /    ", "|===| ", "      "],
            ' ' => vec!["      ", "      ", "      ", "      ", "      ", "      "],
            _ => vec!["[???] ", "[???] ", "[???] ", "[???] ", "[???] ", "      "],
        }
    }

    fn render_mini(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec![" /\\ ", "/--\\", "    "],
            'B' => vec!["|=\\ ", "|=/ ", "    "],
            'C' => vec!["/== ", "\\== ", "    "],
            'D' => vec!["|=\\ ", "|=/ ", "    "],
            'E' => vec!["|== ", "|== ", "    "],
            'F' => vec!["|== ", "|   ", "    "],
            'G' => vec!["/== ", "\\=| ", "    "],
            'H' => vec!["|-| ", "| | ", "    "],
            'I' => vec!["=|= ", "=|= ", "    "],
            'J' => vec!["==| ", "\\=| ", "    "],
            'K' => vec!["|/ ", "|\\  ", "    "],
            'L' => vec!["|   ", "|== ", "    "],
            'M' => vec!["|v| ", "| | ", "    "],
            'N' => vec!["|\\| ", "| | ", "    "],
            'O' => vec!["/=\\ ", "\\=/ ", "    "],
            'P' => vec!["|=\\ ", "|   ", "    "],
            'Q' => vec!["/=\\ ", "\\=\\|", "    "],
            'R' => vec!["|=\\ ", "| \\ ", "    "],
            'S' => vec!["/=  ", "\\=/ ", "    "],
            'T' => vec!["=|= ", " |  ", "    "],
            'U' => vec!["| | ", "\\=/ ", "    "],
            'V' => vec!["| | ", " V  ", "    "],
            'W' => vec!["| | ", "|^| ", "    "],
            'X' => vec!["\\/  ", "/\\  ", "    "],
            'Y' => vec!["\\/  ", " |  ", "    "],
            'Z' => vec!["==/ ", "/== ", "    "],
            ' ' => vec!["    ", "    ", "    "],
            _ => vec!["[?] ", "[?] ", "    "],
        }
    }

    fn render_slant(&self, ch: char) -> Vec<&'static str> {
        match ch {
            'A' => vec!["   /| ", "  /_| ", " /  | ", "/   | ", "      "],
            'B' => vec!["|===  ", "| __) ", "|  _) ", "|===  ", "      "],
            'C' => vec!["  ___/", " /    ", "|     ", " \\___\\", "      "],
            'D' => vec!["|===  ", "|   \\ ", "|   / ", "|===  ", "      "],
            'E' => vec!["|==== ", "|___  ", "|     ", "|==== ", "      "],
            'F' => vec!["|==== ", "|___  ", "|     ", "|     ", "      "],
            'G' => vec!["  ____", " /    ", "| /_  ", " \\__/ ", "      "],
            'H' => vec!["|   | ", "|===| ", "|   | ", "|   | ", "      "],
            'I' => vec!["  |   ", "  |   ", "  |   ", "  |   ", "      "],
            'J' => vec!["    | ", "    | ", " \\  | ", "  \\=/ ", "      "],
            'K' => vec!["|  /  ", "|-<   ", "|  \\  ", "|   \\ ", "      "],
            'L' => vec!["|     ", "|     ", "|     ", "|==== ", "      "],
            'M' => vec!["|\\  /|", "| \\/ |", "|    |", "|    |", "      "],
            'N' => vec!["|\\   |", "| \\  |", "|  \\ |", "|   \\|", "      "],
            'O' => vec!["  __  ", " /  \\ ", "|    |", " \\__/ ", "      "],
            'P' => vec!["|===\\ ", "|   | ", "|===/ ", "|     ", "      "],
            'Q' => vec!["  __  ", " /  \\ ", "|  \\ |", " \\__\\/", "      "],
            'R' => vec!["|===\\ ", "|   | ", "|===/ ", "|   \\ ", "      "],
            'S' => vec!["  ____", " (    ", "  === ", " ____)", "      "],
            'T' => vec!["====| ", "   |  ", "   |  ", "   |  ", "      "],
            'U' => vec!["|   | ", "|   | ", "|   | ", " \\=/ ", "      "],
            'V' => vec!["|   | ", " \\ /  ", "  |   ", "  .   ", "      "],
            'W' => vec!["|    |", "|/\\/\\|", "|    |", ".    .", "      "],
            'X' => vec!["\\   / ", " \\ /  ", " / \\  ", "/   \\ ", "      "],
            'Y' => vec!["\\   / ", " \\ /  ", "  |   ", "  |   ", "      "],
            'Z' => vec!["=====|", "    / ", "   /  ", "|=====", "      "],
            ' ' => vec!["      ", "      ", "      ", "      ", "      "],
            _ => vec!["[????]", "[????]", "[????]", "[????]", "      "],
        }
    }

    fn render_doom(&self, ch: char) -> Vec<&'static str> {
        // Doom-style large chunky letters
        match ch {
            'A' => vec![
                "   ██   ",
                "  ████  ",
                " ██  ██ ",
                "██    ██",
                "████████",
                "██    ██",
                "██    ██",
                "        ",
            ],
            'B' => vec![
                "██████  ",
                "██   ██ ",
                "██   ██ ",
                "██████  ",
                "██   ██ ",
                "██   ██ ",
                "██████  ",
                "        ",
            ],
            'C' => vec![
                " ██████ ",
                "██      ",
                "██      ",
                "██      ",
                "██      ",
                "██      ",
                " ██████ ",
                "        ",
            ],
            'D' => vec![
                "██████  ",
                "██   ██ ",
                "██    ██",
                "██    ██",
                "██    ██",
                "██   ██ ",
                "██████  ",
                "        ",
            ],
            'E' => vec![
                "████████",
                "██      ",
                "██      ",
                "██████  ",
                "██      ",
                "██      ",
                "████████",
                "        ",
            ],
            'F' => vec![
                "████████",
                "██      ",
                "██      ",
                "██████  ",
                "██      ",
                "██      ",
                "██      ",
                "        ",
            ],
            ' ' => vec![
                "        ", "        ", "        ", "        ", "        ", "        ", "        ",
                "        ",
            ],
            _ => vec![
                "████████",
                "██    ██",
                "██    ██",
                "██    ██",
                "██    ██",
                "██    ██",
                "████████",
                "        ",
            ],
        }
    }

    fn render_small_caps(&self, ch: char) -> Vec<&'static str> {
        // Unicode small caps
        match ch {
            'A' => vec!["ᴀ"],
            'B' => vec!["ʙ"],
            'C' => vec!["ᴄ"],
            'D' => vec!["ᴅ"],
            'E' => vec!["ᴇ"],
            'F' => vec!["ꜰ"],
            'G' => vec!["ɢ"],
            'H' => vec!["ʜ"],
            'I' => vec!["ɪ"],
            'J' => vec!["ᴊ"],
            'K' => vec!["ᴋ"],
            'L' => vec!["ʟ"],
            'M' => vec!["ᴍ"],
            'N' => vec!["ɴ"],
            'O' => vec!["ᴏ"],
            'P' => vec!["ᴘ"],
            'Q' => vec!["ǫ"],
            'R' => vec!["ʀ"],
            'S' => vec!["ꜱ"],
            'T' => vec!["ᴛ"],
            'U' => vec!["ᴜ"],
            'V' => vec!["ᴠ"],
            'W' => vec!["ᴡ"],
            'X' => vec!["x"],
            'Y' => vec!["ʏ"],
            'Z' => vec!["ᴢ"],
            ' ' => vec![" "],
            _ => vec!["?"],
        }
    }

    /// Render to frame at position with optional effects.
    pub fn render_at(&self, x: u16, y: u16, frame: &mut Frame, time: f64) {
        let lines = self.render_lines();
        let total_width: usize = lines.first().map(|l| l.chars().count()).unwrap_or(0);

        for (row, line) in lines.iter().enumerate() {
            let py = y.saturating_add(row as u16);
            for (col, ch) in line.chars().enumerate() {
                let px = x.saturating_add(col as u16);

                // Determine color
                let color = if let Some(ref gradient) = self.gradient {
                    let t = if total_width > 1 {
                        (col as f64 / (total_width - 1) as f64 + time * 0.2).rem_euclid(1.0)
                    } else {
                        0.5
                    };
                    gradient.sample(t)
                } else {
                    self.color.unwrap_or(PackedRgba::rgb(255, 255, 255))
                };

                if let Some(cell) = frame.buffer.get_mut(px, py) {
                    cell.content = CellContent::from_char(ch);
                    if ch != ' ' {
                        cell.fg = color;
                    }
                }
            }
        }
    }
}

// =============================================================================
// Sparkle Effect - Particles that twinkle
// =============================================================================

/// A single sparkle particle.
#[derive(Debug, Clone)]
pub struct Sparkle {
    pub x: f64,
    pub y: f64,
    pub brightness: f64,
    pub phase: f64,
}

/// Manages a collection of sparkle effects.
#[derive(Debug, Clone, Default)]
pub struct SparkleField {
    sparkles: Vec<Sparkle>,
    density: f64,
}

impl SparkleField {
    /// Create a new sparkle field.
    pub fn new(density: f64) -> Self {
        Self {
            sparkles: Vec::new(),
            density: density.clamp(0.0, 1.0),
        }
    }

    /// Initialize sparkles for an area.
    pub fn init_for_area(&mut self, width: u16, height: u16, seed: u64) {
        self.sparkles.clear();
        let count = ((width as f64 * height as f64) * self.density * 0.05) as usize;

        let mut rng = seed;
        for _ in 0..count {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (rng % width as u64) as f64;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y = (rng % height as u64) as f64;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let phase = (rng % 1000) as f64 / 1000.0 * TAU;

            self.sparkles.push(Sparkle {
                x,
                y,
                brightness: 1.0,
                phase,
            });
        }
    }

    /// Update sparkles for animation.
    pub fn update(&mut self, time: f64) {
        for sparkle in &mut self.sparkles {
            sparkle.brightness = 0.5 + 0.5 * (time * 3.0 + sparkle.phase).sin();
        }
    }

    /// Render sparkles to frame.
    pub fn render(&self, offset_x: u16, offset_y: u16, frame: &mut Frame) {
        for sparkle in &self.sparkles {
            let px = offset_x.saturating_add(sparkle.x as u16);
            let py = offset_y.saturating_add(sparkle.y as u16);

            if let Some(cell) = frame.buffer.get_mut(px, py) {
                let b = (sparkle.brightness * 255.0) as u8;
                // Use star characters for sparkle
                let ch = if sparkle.brightness > 0.8 {
                    '*'
                } else if sparkle.brightness > 0.5 {
                    '+'
                } else {
                    '.'
                };
                cell.content = CellContent::from_char(ch);
                cell.fg = PackedRgba::rgb(b, b, b.saturating_add(50));
            }
        }
    }
}

// =============================================================================
// Matrix/Cyber Characters
// =============================================================================

/// Characters for matrix/cyber style effects.
pub struct CyberChars;

impl CyberChars {
    /// Get a random cyber character based on seed.
    pub fn get(seed: u64) -> char {
        const CYBER_CHARS: &[char] = &[
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'ア', 'イ', 'ウ', 'エ', 'オ', 'カ',
            'キ', 'ク', 'ケ', 'コ', 'サ', 'シ', 'ス', 'セ', 'ソ', 'タ', 'チ', 'ツ', 'テ', 'ト',
            '/', '\\', '|', '-', '+', '*', '#', '@', '=', '>', '<', '[', ']', '{', '}', '(', ')',
            '$', '%', '&',
        ];
        let idx = (seed % CYBER_CHARS.len() as u64) as usize;
        CYBER_CHARS[idx]
    }

    /// Get a random printable ASCII character.
    pub fn ascii(seed: u64) -> char {
        let code = 33 + (seed % 94) as u8;
        code as char
    }
}
