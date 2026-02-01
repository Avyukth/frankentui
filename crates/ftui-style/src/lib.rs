#![forbid(unsafe_code)]

//! Style types for FrankenTUI with CSS-like cascading semantics.
//!
//! This crate provides:
//! - [`Style`] for unified text styling with CSS-like inheritance
//! - [`StyleSheet`] for named style registration (CSS-like classes)
//! - [`Theme`] for semantic color slots with light/dark mode support
//! - Color types and downgrade utilities

pub mod color;
pub mod style;
pub mod stylesheet;
pub mod theme;

pub use color::{Ansi16, Color, ColorCache, ColorProfile, MonoColor, Rgb};
pub use style::{Style, StyleFlags};
pub use stylesheet::{StyleId, StyleSheet};
pub use theme::{AdaptiveColor, ResolvedTheme, Theme, ThemeBuilder};
