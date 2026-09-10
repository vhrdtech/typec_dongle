//! Donguru Colour theme and styles.
//!
//! Donguru uses a 24-bit colour theme/style and a 16-colour fallback theme style
//! for terminals without truecolor support.

use clap::builder::styling::{AnsiColor, Color, Effects, RgbColor, Style, Styles};

/// A palette of colours for the elements clap can style.
pub struct Palette {
    header: Color,
    usage: Color,
    literal: Color,
    placeholder: Color,
    valid: Color,
    invalid: Color,
    error: Color,
}

impl Palette {
    /// Section headers, e.g. "Options:".
    pub const fn header(&self) -> Color {
        self.header
    }

    /// The usage line.
    pub const fn usage(&self) -> Color {
        self.usage
    }

    /// Flags, options and subcommand names.
    pub const fn literal(&self) -> Color {
        self.literal
    }

    /// Value placeholders, e.g. "<FILE>".
    pub const fn placeholder(&self) -> Color {
        self.placeholder
    }

    /// Valid values, e.g. suggestions.
    pub const fn valid(&self) -> Color {
        self.valid
    }

    /// Invalid values.
    pub const fn invalid(&self) -> Color {
        self.invalid
    }

    /// Error messages.
    pub const fn error(&self) -> Color {
        self.error
    }
}

impl From<&Palette> for Styles {
    fn from(palette: &Palette) -> Self {
        /// A bold style in `color`.
        const fn bold(color: Color) -> Style {
            Style::new().fg_color(Some(color)).effects(Effects::BOLD)
        }

        /// A bold, underlined style in `color`.
        const fn bold_under(color: Color) -> Style {
            const BOLD_UNDER: Effects = Effects::BOLD.insert(Effects::UNDERLINE);
            Style::new().fg_color(Some(color)).effects(BOLD_UNDER)
        }

        Styles::styled()
            .header(bold_under(palette.header))
            .usage(bold(palette.usage))
            .literal(bold(palette.literal))
            .placeholder(Style::new().fg_color(Some(palette.placeholder)))
            .valid(bold(palette.valid))
            .invalid(bold(palette.invalid))
            .error(bold_under(palette.error))
    }
}

impl From<Palette> for Styles {
    fn from(palette: Palette) -> Self {
        Self::from(&palette)
    }
}

/// An RGB [`Color`].
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(RgbColor(r, g, b))
}

/// 24-bit palette
pub const DEFAULT_PALETTE: Palette = Palette {
    header: rgb(0xFF, 0x9E, 0x3B),      // warm acorn orange
    usage: rgb(0x5E, 0xE6, 0xC0),       // mint green
    literal: rgb(0xFF, 0xD8, 0x66),     // soft amber yellow
    placeholder: rgb(0x9C, 0x8C, 0xFF), // pale violet
    valid: rgb(0xA6, 0xE2, 0x2E),       // leaf green
    invalid: rgb(0xFF, 0x61, 0x88),     // bright pink
    error: rgb(0xFF, 0x38, 0x60),       // vivid crimson
};

/// 16-colour approximation of [`DEFAULT_PALETTE`].
pub const FALLBACK_PALETTE: Palette = Palette {
    header: Color::Ansi(AnsiColor::Yellow),
    usage: Color::Ansi(AnsiColor::BrightCyan),
    literal: Color::Ansi(AnsiColor::BrightYellow),
    placeholder: Color::Ansi(AnsiColor::BrightMagenta),
    valid: Color::Ansi(AnsiColor::BrightGreen),
    invalid: Color::Ansi(AnsiColor::BrightRed),
    error: Color::Ansi(AnsiColor::Red),
};

/// Truecolor style.
pub fn donguru_style() -> Styles {
    (&DEFAULT_PALETTE).into()
}

/// 16-colour style for terminals that mangle 24-bit colour.
pub fn fallback_style() -> Styles {
    (&FALLBACK_PALETTE).into()
}

/// Best-effort truecolor detection (`COLORTERM`, then known-good `TERM` values).
fn truecolor() -> bool {
    if let Ok(colorterm) = std::env::var("COLORTERM")
        && (colorterm.contains("truecolor") || colorterm.contains("24bit"))
    {
        return true;
    }
    std::env::var("TERM").is_ok_and(|term| {
        term.contains("truecolor") || term.contains("direct") || term.contains("24bit")
    })
}

/// Pick the richest palette the current terminal can render.
pub fn select_style() -> Styles {
    if truecolor() {
        donguru_style()
    } else {
        fallback_style()
    }
}
