//! Donguru Colour theme and styles.
//!
//! Donguru uses a 24-bit colour theme/style and a 16-colour fallback theme style
//! for terminals without truecolor support.

use crate::cli::ColorMode;
use clap::ColorChoice;
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

/// The roles of text in command output (`--format text` on stdout), painted from the palette. Without colour
/// every role prints the plain text, so a renderer looks the same piped as on a terminal, only without colour.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    colour: bool,
    truecolor: bool,
}

impl Theme {
    /// Colour per `--color`, `NO_COLOR` and whether stdout is a terminal (see [colour_enabled]).
    pub fn for_stdout(mode: ColorMode) -> Self {
        use std::io::IsTerminal;
        Self::new(colour_enabled(mode, std::io::stdout().is_terminal()))
    }

    /// `colour` on or off; the palette (24-bit or 16 colours) follows the terminal.
    pub fn new(colour: bool) -> Self {
        Theme {
            colour,
            truecolor: colour && truecolor(),
        }
    }

    /// No colour at all: json, tests, snapshots.
    pub const fn plain() -> Self {
        Theme {
            colour: false,
            truecolor: false,
        }
    }

    pub const fn colour(&self) -> bool {
        self.colour
    }

    fn palette(&self) -> &'static Palette {
        if self.truecolor {
            &DEFAULT_PALETTE
        } else {
            &FALLBACK_PALETTE
        }
    }

    fn paint<T: std::fmt::Display>(&self, style: Style, value: T) -> Painted<T> {
        Painted {
            style: self.colour.then_some(style),
            value,
        }
    }

    /// A row label of a key / value table (`vbus`, `serial`).
    pub fn key<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(Style::new().fg_color(Some(self.palette().header())), value)
    }

    /// A section or a group of rows (`plug`, `▶ pass 1/1`).
    pub fn heading<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(
            Style::new()
                .fg_color(Some(self.palette().header()))
                .effects(Effects::BOLD),
            value,
        )
    }

    /// A good state: on, connected, passed.
    pub fn ok<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(Style::new().fg_color(Some(self.palette().valid())), value)
    }

    /// A state worth a look but not a failure: a warning, a non-zero counter.
    pub fn warn<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(Style::new().fg_color(Some(self.palette().literal())), value)
    }

    /// A bad state: tripped, fault, FAIL.
    pub fn bad<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(
            Style::new()
                .fg_color(Some(self.palette().error()))
                .effects(Effects::BOLD),
            value,
        )
    }

    /// A measured or configured value: volts, a serial number, a version.
    pub fn value<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(Style::new().fg_color(Some(self.palette().usage())), value)
    }

    /// Something absent or secondary: "(not implemented by the device's firmware)", units, notes.
    pub fn dim<T: std::fmt::Display>(&self, value: T) -> Painted<T> {
        self.paint(Style::new().effects(Effects::DIMMED), value)
    }
}

/// A value in a [Theme] role. Width, alignment and precision apply to the text itself (`{:<8}` pads the
/// visible text, not the escape codes).
pub struct Painted<T> {
    style: Option<Style>,
    value: T,
}

impl<T: std::fmt::Display> std::fmt::Display for Painted<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::{Alignment, Write};
        let text = match f.precision() {
            Some(p) => format!("{:.*}", p, self.value),
            None => self.value.to_string(),
        };
        let gap = f.width().unwrap_or(0).saturating_sub(text.chars().count());
        let (before, after) = match f.align() {
            None | Some(Alignment::Left) => (0, gap),
            Some(Alignment::Right) => (gap, 0),
            Some(Alignment::Center) => (gap / 2, gap - gap / 2),
        };
        let fill = f.fill();
        for _ in 0..before {
            f.write_char(fill)?;
        }
        match self.style {
            None => f.write_str(&text)?,
            Some(style) => write!(f, "{style}{text}{style:#}")?,
        }
        for _ in 0..after {
            f.write_char(fill)?;
        }
        Ok(())
    }
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

/// Whether to emit colour: `--color always|never` decide, `auto` colours a terminal unless `NO_COLOR` is set (to
/// anything non-empty, see no-color.org) or `TERM=dumb`.
pub fn colour_enabled(mode: ColorMode, is_terminal: bool) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            is_terminal
                && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
                && std::env::var_os("TERM").is_none_or(|t| t != "dumb")
        }
    }
}

/// `--color WHEN` / `--color=WHEN` from the raw arguments, for clap's own output (help, usage errors), which is
/// rendered before the global flag is parsed. Without it clap decides itself (terminal, `NO_COLOR`).
pub fn choice_from_args(args: impl IntoIterator<Item = String>) -> ColorChoice {
    let mut args = args.into_iter().skip(1);
    let mut value = None;
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if arg == "--color" {
            value = args.next();
        } else if let Some(v) = arg.strip_prefix("--color=") {
            value = Some(v.to_string());
        }
    }
    match value.as_deref() {
        Some("always") => ColorChoice::Always,
        Some("never") => ColorChoice::Never,
        _ => ColorChoice::Auto,
    }
}

/// Pick the richest palette the current terminal can render.
pub fn select_style() -> Styles {
    if truecolor() {
        donguru_style()
    } else {
        fallback_style()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn color_flag_is_found_anywhere() {
        assert_eq!(
            choice_from_args(args(&["dg", "--color", "never", "info"])),
            ColorChoice::Never
        );
        assert_eq!(
            choice_from_args(args(&["dg", "info", "--color=always"])),
            ColorChoice::Always
        );
        assert_eq!(choice_from_args(args(&["dg", "info"])), ColorChoice::Auto);
        assert_eq!(
            choice_from_args(args(&["dg", "--", "--color=never"])),
            ColorChoice::Auto
        );
    }

    #[test]
    fn painted_pads_the_text_not_the_codes() {
        let plain = Theme::plain();
        assert_eq!(format!("[{:<5}]", plain.key("ab")), "[ab   ]");
        assert_eq!(format!("[{:>6.2}]", plain.value(1.23456)), "[  1.23]");
        let colour = Theme::new(true);
        let s = format!("[{:<5}]", colour.ok("ab"));
        assert!(s.starts_with("[\x1b["), "{s:?}");
        assert!(s.ends_with("ab\x1b[0m   ]"), "{s:?}");
    }

    #[test]
    fn forced_modes_ignore_the_terminal() {
        assert!(colour_enabled(ColorMode::Always, false));
        assert!(!colour_enabled(ColorMode::Never, true));
    }
}
