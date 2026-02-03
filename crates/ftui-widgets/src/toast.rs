#![forbid(unsafe_code)]

//! Toast widget for displaying transient notifications.
//!
//! A toast is a non-blocking notification that appears temporarily and
//! can be dismissed automatically or manually. Toasts support:
//!
//! - Multiple positions (corners and center top/bottom)
//! - Automatic dismissal with configurable duration
//! - Icons for different message types (success, error, warning, info)
//! - Semantic styling that integrates with the theme system
//!
//! # Example
//!
//! ```ignore
//! let toast = Toast::new("File saved successfully")
//!     .icon(ToastIcon::Success)
//!     .position(ToastPosition::TopRight)
//!     .duration(Duration::from_secs(3));
//! ```

use std::time::{Duration, Instant};

use ftui_core::geometry::Rect;
use ftui_render::cell::Cell;
use ftui_render::frame::Frame;
use ftui_style::Style;
use unicode_width::UnicodeWidthStr;

use crate::{Widget, set_style_area};

/// Unique identifier for a toast notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(pub u64);

impl ToastId {
    /// Create a new toast ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Position where the toast should be displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastPosition {
    /// Top-left corner.
    TopLeft,
    /// Top center.
    TopCenter,
    /// Top-right corner.
    #[default]
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom center.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

impl ToastPosition {
    /// Calculate the toast's top-left position within a terminal area.
    ///
    /// Returns `(x, y)` for the toast's origin given its dimensions.
    pub fn calculate_position(
        self,
        terminal_width: u16,
        terminal_height: u16,
        toast_width: u16,
        toast_height: u16,
        margin: u16,
    ) -> (u16, u16) {
        let x = match self {
            Self::TopLeft | Self::BottomLeft => margin,
            Self::TopCenter | Self::BottomCenter => terminal_width.saturating_sub(toast_width) / 2,
            Self::TopRight | Self::BottomRight => terminal_width
                .saturating_sub(toast_width)
                .saturating_sub(margin),
        };

        let y = match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => margin,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => terminal_height
                .saturating_sub(toast_height)
                .saturating_sub(margin),
        };

        (x, y)
    }
}

/// Icon displayed in the toast to indicate message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastIcon {
    /// Success indicator (checkmark).
    Success,
    /// Error indicator (X mark).
    Error,
    /// Warning indicator (exclamation).
    Warning,
    /// Information indicator (i).
    #[default]
    Info,
    /// Custom single character.
    Custom(char),
}

impl ToastIcon {
    /// Get the display character for this icon.
    pub fn as_char(self) -> char {
        match self {
            Self::Success => '\u{2713}', // ✓
            Self::Error => '\u{2717}',   // ✗
            Self::Warning => '!',
            Self::Info => 'i',
            Self::Custom(c) => c,
        }
    }

    /// Get the fallback ASCII character for degraded rendering.
    pub fn as_ascii(self) -> char {
        match self {
            Self::Success => '+',
            Self::Error => 'x',
            Self::Warning => '!',
            Self::Info => 'i',
            Self::Custom(c) if c.is_ascii() => c,
            Self::Custom(_) => '*',
        }
    }
}

/// Visual style variant for the toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastStyle {
    /// Success style (typically green).
    Success,
    /// Error style (typically red).
    Error,
    /// Warning style (typically yellow/orange).
    Warning,
    /// Informational style (typically blue).
    #[default]
    Info,
    /// Neutral style (no semantic coloring).
    Neutral,
}

/// Configuration for a toast notification.
#[derive(Debug, Clone)]
pub struct ToastConfig {
    /// Position on screen.
    pub position: ToastPosition,
    /// Auto-dismiss duration. `None` means persistent until dismissed.
    pub duration: Option<Duration>,
    /// Visual style variant.
    pub style_variant: ToastStyle,
    /// Maximum width in columns.
    pub max_width: u16,
    /// Margin from screen edges.
    pub margin: u16,
    /// Whether the toast can be dismissed by the user.
    pub dismissable: bool,
}

impl Default for ToastConfig {
    fn default() -> Self {
        Self {
            position: ToastPosition::default(),
            duration: Some(Duration::from_secs(5)),
            style_variant: ToastStyle::default(),
            max_width: 50,
            margin: 1,
            dismissable: true,
        }
    }
}

/// Content of a toast notification.
#[derive(Debug, Clone)]
pub struct ToastContent {
    /// Main message text.
    pub message: String,
    /// Optional icon.
    pub icon: Option<ToastIcon>,
    /// Optional title.
    pub title: Option<String>,
}

impl ToastContent {
    /// Create new content with just a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            icon: None,
            title: None,
        }
    }

    /// Set the icon.
    pub fn with_icon(mut self, icon: ToastIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// Internal state tracking for a toast.
#[derive(Debug, Clone)]
pub struct ToastState {
    /// When the toast was created.
    pub created_at: Instant,
    /// Whether the toast has been dismissed.
    pub dismissed: bool,
}

impl Default for ToastState {
    fn default() -> Self {
        Self {
            created_at: Instant::now(),
            dismissed: false,
        }
    }
}

/// A toast notification widget.
///
/// Toasts display transient messages to the user, typically in a corner
/// of the screen. They can auto-dismiss after a duration or be manually
/// dismissed.
///
/// # Example
///
/// ```ignore
/// let toast = Toast::new("Operation completed")
///     .icon(ToastIcon::Success)
///     .position(ToastPosition::TopRight)
///     .duration(Duration::from_secs(3));
///
/// // Render the toast
/// toast.render(area, frame);
/// ```
#[derive(Debug, Clone)]
pub struct Toast {
    /// Unique identifier.
    pub id: ToastId,
    /// Toast content.
    pub content: ToastContent,
    /// Configuration.
    pub config: ToastConfig,
    /// Internal state.
    pub state: ToastState,
    /// Base style override.
    style: Style,
    /// Icon style override.
    icon_style: Style,
    /// Title style override.
    title_style: Style,
}

impl Toast {
    /// Create a new toast with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = ToastId::new(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));

        Self {
            id,
            content: ToastContent::new(message),
            config: ToastConfig::default(),
            state: ToastState::default(),
            style: Style::default(),
            icon_style: Style::default(),
            title_style: Style::default(),
        }
    }

    /// Create a toast with a specific ID.
    pub fn with_id(id: ToastId, message: impl Into<String>) -> Self {
        Self {
            id,
            content: ToastContent::new(message),
            config: ToastConfig::default(),
            state: ToastState::default(),
            style: Style::default(),
            icon_style: Style::default(),
            title_style: Style::default(),
        }
    }

    // --- Builder methods ---

    /// Set the toast icon.
    pub fn icon(mut self, icon: ToastIcon) -> Self {
        self.content.icon = Some(icon);
        self
    }

    /// Set the toast title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.content.title = Some(title.into());
        self
    }

    /// Set the toast position.
    pub fn position(mut self, position: ToastPosition) -> Self {
        self.config.position = position;
        self
    }

    /// Set the auto-dismiss duration.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.config.duration = Some(duration);
        self
    }

    /// Make the toast persistent (no auto-dismiss).
    pub fn persistent(mut self) -> Self {
        self.config.duration = None;
        self
    }

    /// Set the style variant.
    pub fn style_variant(mut self, variant: ToastStyle) -> Self {
        self.config.style_variant = variant;
        self
    }

    /// Set the maximum width.
    pub fn max_width(mut self, width: u16) -> Self {
        self.config.max_width = width;
        self
    }

    /// Set the margin from screen edges.
    pub fn margin(mut self, margin: u16) -> Self {
        self.config.margin = margin;
        self
    }

    /// Set whether the toast is dismissable.
    pub fn dismissable(mut self, dismissable: bool) -> Self {
        self.config.dismissable = dismissable;
        self
    }

    /// Set the base style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the icon style.
    pub fn with_icon_style(mut self, style: Style) -> Self {
        self.icon_style = style;
        self
    }

    /// Set the title style.
    pub fn with_title_style(mut self, style: Style) -> Self {
        self.title_style = style;
        self
    }

    // --- State methods ---

    /// Check if the toast has expired based on its duration.
    pub fn is_expired(&self) -> bool {
        if let Some(duration) = self.config.duration {
            self.state.created_at.elapsed() >= duration
        } else {
            false
        }
    }

    /// Check if the toast should be visible.
    pub fn is_visible(&self) -> bool {
        !self.state.dismissed && !self.is_expired()
    }

    /// Dismiss the toast.
    pub fn dismiss(&mut self) {
        self.state.dismissed = true;
    }

    /// Get the remaining time before auto-dismiss.
    pub fn remaining_time(&self) -> Option<Duration> {
        self.config.duration.map(|d| {
            let elapsed = self.state.created_at.elapsed();
            d.saturating_sub(elapsed)
        })
    }

    /// Calculate the toast dimensions based on content.
    pub fn calculate_dimensions(&self) -> (u16, u16) {
        let max_width = self.config.max_width as usize;

        // Calculate content width
        let icon_width = if self.content.icon.is_some() { 2 } else { 0 }; // icon + space
        let message_width = UnicodeWidthStr::width(self.content.message.as_str());
        let title_width = self
            .content
            .title
            .as_ref()
            .map(|t| UnicodeWidthStr::width(t.as_str()))
            .unwrap_or(0);

        // Content width is max of title and message (plus icon)
        let content_width = (icon_width + message_width).max(title_width);

        // Add padding (1 char each side) and border (1 char each side)
        let total_width = content_width.saturating_add(4).min(max_width);

        // Height: border (2) + optional title (1) + message (1) + padding (0)
        let has_title = self.content.title.is_some();
        let height = if has_title { 4 } else { 3 };

        (total_width as u16, height as u16)
    }
}

impl Widget for Toast {
    fn render(&self, area: Rect, frame: &mut Frame) {
        #[cfg(feature = "tracing")]
        let _span = tracing::debug_span!(
            "widget_render",
            widget = "Toast",
            x = area.x,
            y = area.y,
            w = area.width,
            h = area.height
        )
        .entered();

        if area.is_empty() || !self.is_visible() {
            return;
        }

        let deg = frame.buffer.degradation;

        // Calculate actual render area (use provided area or calculate from content)
        let (content_width, content_height) = self.calculate_dimensions();
        let width = area.width.min(content_width);
        let height = area.height.min(content_height);

        if width < 3 || height < 3 {
            return; // Too small to render
        }

        let render_area = Rect::new(area.x, area.y, width, height);

        // Apply base style to the entire area
        if deg.apply_styling() {
            set_style_area(&mut frame.buffer, render_area, self.style);
        }

        // Draw border
        let use_unicode = deg.apply_styling();
        let (tl, tr, bl, br, h, v) = if use_unicode {
            (
                '\u{250C}', '\u{2510}', '\u{2514}', '\u{2518}', '\u{2500}', '\u{2502}',
            )
        } else {
            ('+', '+', '+', '+', '-', '|')
        };

        // Top border
        if let Some(cell) = frame.buffer.get_mut(render_area.x, render_area.y) {
            *cell = Cell::from_char(tl);
            if deg.apply_styling() {
                crate::apply_style(cell, self.style);
            }
        }
        for x in (render_area.x + 1)..(render_area.right().saturating_sub(1)) {
            if let Some(cell) = frame.buffer.get_mut(x, render_area.y) {
                *cell = Cell::from_char(h);
                if deg.apply_styling() {
                    crate::apply_style(cell, self.style);
                }
            }
        }
        if let Some(cell) = frame
            .buffer
            .get_mut(render_area.right().saturating_sub(1), render_area.y)
        {
            *cell = Cell::from_char(tr);
            if deg.apply_styling() {
                crate::apply_style(cell, self.style);
            }
        }

        // Bottom border
        let bottom_y = render_area.bottom().saturating_sub(1);
        if let Some(cell) = frame.buffer.get_mut(render_area.x, bottom_y) {
            *cell = Cell::from_char(bl);
            if deg.apply_styling() {
                crate::apply_style(cell, self.style);
            }
        }
        for x in (render_area.x + 1)..(render_area.right().saturating_sub(1)) {
            if let Some(cell) = frame.buffer.get_mut(x, bottom_y) {
                *cell = Cell::from_char(h);
                if deg.apply_styling() {
                    crate::apply_style(cell, self.style);
                }
            }
        }
        if let Some(cell) = frame
            .buffer
            .get_mut(render_area.right().saturating_sub(1), bottom_y)
        {
            *cell = Cell::from_char(br);
            if deg.apply_styling() {
                crate::apply_style(cell, self.style);
            }
        }

        // Side borders
        for y in (render_area.y + 1)..bottom_y {
            if let Some(cell) = frame.buffer.get_mut(render_area.x, y) {
                *cell = Cell::from_char(v);
                if deg.apply_styling() {
                    crate::apply_style(cell, self.style);
                }
            }
            if let Some(cell) = frame
                .buffer
                .get_mut(render_area.right().saturating_sub(1), y)
            {
                *cell = Cell::from_char(v);
                if deg.apply_styling() {
                    crate::apply_style(cell, self.style);
                }
            }
        }

        // Draw content
        let content_x = render_area.x + 1; // After left border
        let content_width = width.saturating_sub(2); // Minus borders
        let mut content_y = render_area.y + 1;

        // Draw title if present
        if let Some(ref title) = self.content.title {
            let title_style = if deg.apply_styling() {
                self.title_style.merge(&self.style)
            } else {
                Style::default()
            };

            for (i, c) in title.chars().enumerate() {
                if i as u16 >= content_width {
                    break;
                }
                if let Some(cell) = frame.buffer.get_mut(content_x + i as u16, content_y) {
                    *cell = Cell::from_char(c);
                    if deg.apply_styling() {
                        crate::apply_style(cell, title_style);
                    }
                }
            }
            content_y += 1;
        }

        // Draw icon and message
        let mut msg_x = content_x;

        if let Some(icon) = self.content.icon {
            let icon_char = if use_unicode {
                icon.as_char()
            } else {
                icon.as_ascii()
            };

            if let Some(cell) = frame.buffer.get_mut(msg_x, content_y) {
                *cell = Cell::from_char(icon_char);
                if deg.apply_styling() {
                    let icon_style = self.icon_style.merge(&self.style);
                    crate::apply_style(cell, icon_style);
                }
            }
            msg_x += 1;

            // Space after icon
            if let Some(cell) = frame.buffer.get_mut(msg_x, content_y) {
                *cell = Cell::from_char(' ');
            }
            msg_x += 1;
        }

        // Draw message
        let remaining_width = content_width.saturating_sub(msg_x - content_x);
        for (i, c) in self.content.message.chars().enumerate() {
            if i as u16 >= remaining_width {
                break;
            }
            if let Some(cell) = frame.buffer.get_mut(msg_x + i as u16, content_y) {
                *cell = Cell::from_char(c);
                if deg.apply_styling() {
                    crate::apply_style(cell, self.style);
                }
            }
        }
    }

    fn is_essential(&self) -> bool {
        // Toasts are informational, not essential
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ftui_render::grapheme_pool::GraphemePool;

    #[test]
    fn test_toast_new() {
        let toast = Toast::new("Hello");
        assert_eq!(toast.content.message, "Hello");
        assert!(toast.content.icon.is_none());
        assert!(toast.content.title.is_none());
        assert!(toast.is_visible());
    }

    #[test]
    fn test_toast_builder() {
        let toast = Toast::new("Test message")
            .icon(ToastIcon::Success)
            .title("Success")
            .position(ToastPosition::BottomRight)
            .duration(Duration::from_secs(10))
            .max_width(60);

        assert_eq!(toast.content.message, "Test message");
        assert_eq!(toast.content.icon, Some(ToastIcon::Success));
        assert_eq!(toast.content.title, Some("Success".to_string()));
        assert_eq!(toast.config.position, ToastPosition::BottomRight);
        assert_eq!(toast.config.duration, Some(Duration::from_secs(10)));
        assert_eq!(toast.config.max_width, 60);
    }

    #[test]
    fn test_toast_persistent() {
        let toast = Toast::new("Persistent").persistent();
        assert!(toast.config.duration.is_none());
        assert!(!toast.is_expired());
    }

    #[test]
    fn test_toast_dismiss() {
        let mut toast = Toast::new("Dismissable");
        assert!(toast.is_visible());
        toast.dismiss();
        assert!(!toast.is_visible());
        assert!(toast.state.dismissed);
    }

    #[test]
    fn test_toast_position_calculate() {
        let terminal_width = 80;
        let terminal_height = 24;
        let toast_width = 30;
        let toast_height = 3;
        let margin = 1;

        // Top-left
        let (x, y) = ToastPosition::TopLeft.calculate_position(
            terminal_width,
            terminal_height,
            toast_width,
            toast_height,
            margin,
        );
        assert_eq!(x, 1);
        assert_eq!(y, 1);

        // Top-right
        let (x, y) = ToastPosition::TopRight.calculate_position(
            terminal_width,
            terminal_height,
            toast_width,
            toast_height,
            margin,
        );
        assert_eq!(x, 80 - 30 - 1); // 49
        assert_eq!(y, 1);

        // Bottom-right
        let (x, y) = ToastPosition::BottomRight.calculate_position(
            terminal_width,
            terminal_height,
            toast_width,
            toast_height,
            margin,
        );
        assert_eq!(x, 49);
        assert_eq!(y, 24 - 3 - 1); // 20

        // Top-center
        let (x, y) = ToastPosition::TopCenter.calculate_position(
            terminal_width,
            terminal_height,
            toast_width,
            toast_height,
            margin,
        );
        assert_eq!(x, (80 - 30) / 2); // 25
        assert_eq!(y, 1);
    }

    #[test]
    fn test_toast_icon_chars() {
        assert_eq!(ToastIcon::Success.as_char(), '\u{2713}');
        assert_eq!(ToastIcon::Error.as_char(), '\u{2717}');
        assert_eq!(ToastIcon::Warning.as_char(), '!');
        assert_eq!(ToastIcon::Info.as_char(), 'i');
        assert_eq!(ToastIcon::Custom('*').as_char(), '*');

        // ASCII fallbacks
        assert_eq!(ToastIcon::Success.as_ascii(), '+');
        assert_eq!(ToastIcon::Error.as_ascii(), 'x');
    }

    #[test]
    fn test_toast_dimensions() {
        let toast = Toast::new("Short");
        let (w, h) = toast.calculate_dimensions();
        // "Short" = 5 chars + 4 (padding+border) = 9
        assert_eq!(w, 9);
        assert_eq!(h, 3); // No title

        let toast_with_title = Toast::new("Message").title("Title");
        let (_w, h) = toast_with_title.calculate_dimensions();
        assert_eq!(h, 4); // With title
    }

    #[test]
    fn test_toast_dimensions_with_icon() {
        let toast = Toast::new("Message").icon(ToastIcon::Success);
        let (w, _h) = toast.calculate_dimensions();
        // icon(1) + space(1) + "Message"(7) + padding+border(4) = 13
        assert_eq!(w, 13);
    }

    #[test]
    fn test_toast_dimensions_max_width() {
        let toast = Toast::new("This is a very long message that exceeds max width").max_width(20);
        let (w, _h) = toast.calculate_dimensions();
        assert!(w <= 20);
    }

    #[test]
    fn test_toast_render_basic() {
        let toast = Toast::new("Hello");
        let area = Rect::new(0, 0, 15, 5);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(15, 5, &mut pool);
        toast.render(area, &mut frame);

        // Check border corners
        assert_eq!(
            frame.buffer.get(0, 0).unwrap().content.as_char(),
            Some('\u{250C}')
        ); // ┌
        assert!(frame.buffer.get(1, 1).is_some()); // Content area exists
    }

    #[test]
    fn test_toast_render_with_icon() {
        let toast = Toast::new("OK").icon(ToastIcon::Success);
        let area = Rect::new(0, 0, 10, 5);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(10, 5, &mut pool);
        toast.render(area, &mut frame);

        // Icon should be at position (1, 1) - inside border
        let icon_cell = frame.buffer.get(1, 1).unwrap();
        assert_eq!(icon_cell.content.as_char(), Some('\u{2713}')); // ✓
    }

    #[test]
    fn test_toast_render_with_title() {
        let toast = Toast::new("Body").title("Head");
        let area = Rect::new(0, 0, 15, 6);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(15, 6, &mut pool);
        toast.render(area, &mut frame);

        // Title at row 1, message at row 2
        let title_cell = frame.buffer.get(1, 1).unwrap();
        assert_eq!(title_cell.content.as_char(), Some('H'));
    }

    #[test]
    fn test_toast_render_zero_area() {
        let toast = Toast::new("Test");
        let area = Rect::new(0, 0, 0, 0);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(1, 1, &mut pool);
        toast.render(area, &mut frame); // Should not panic
    }

    #[test]
    fn test_toast_render_small_area() {
        let toast = Toast::new("Test");
        let area = Rect::new(0, 0, 2, 2);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(2, 2, &mut pool);
        toast.render(area, &mut frame); // Should not render (too small)
    }

    #[test]
    fn test_toast_not_visible_when_dismissed() {
        let mut toast = Toast::new("Test");
        toast.dismiss();
        let area = Rect::new(0, 0, 20, 5);
        let mut pool = GraphemePool::new();
        let mut frame = Frame::new(20, 5, &mut pool);

        // Save original state
        let original = frame.buffer.get(0, 0).unwrap().content.as_char();

        toast.render(area, &mut frame);

        // Buffer should be unchanged (dismissed toast doesn't render)
        assert_eq!(frame.buffer.get(0, 0).unwrap().content.as_char(), original);
    }

    #[test]
    fn test_toast_is_not_essential() {
        let toast = Toast::new("Test");
        assert!(!toast.is_essential());
    }

    #[test]
    fn test_toast_id_uniqueness() {
        let toast1 = Toast::new("A");
        let toast2 = Toast::new("B");
        assert_ne!(toast1.id, toast2.id);
    }

    #[test]
    fn test_toast_style_variants() {
        let success = Toast::new("OK").style_variant(ToastStyle::Success);
        let error = Toast::new("Fail").style_variant(ToastStyle::Error);
        let warning = Toast::new("Warn").style_variant(ToastStyle::Warning);
        let info = Toast::new("Info").style_variant(ToastStyle::Info);
        let neutral = Toast::new("Neutral").style_variant(ToastStyle::Neutral);

        assert_eq!(success.config.style_variant, ToastStyle::Success);
        assert_eq!(error.config.style_variant, ToastStyle::Error);
        assert_eq!(warning.config.style_variant, ToastStyle::Warning);
        assert_eq!(info.config.style_variant, ToastStyle::Info);
        assert_eq!(neutral.config.style_variant, ToastStyle::Neutral);
    }

    #[test]
    fn test_toast_content_builder() {
        let content = ToastContent::new("Message")
            .with_icon(ToastIcon::Warning)
            .with_title("Alert");

        assert_eq!(content.message, "Message");
        assert_eq!(content.icon, Some(ToastIcon::Warning));
        assert_eq!(content.title, Some("Alert".to_string()));
    }
}
