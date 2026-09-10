//! Example binary to visualize the Donguru colour theme(s).
//!
//! ```console
//! cargo run --example theme_preview
//! cargo run --example theme_preview -- --help
//! ```

use std::io::Write;

use clap::builder::styling::{Color, Reset, Style, Styles};
use clap::{Parser, ValueEnum};

use donguru_cli::theme::{self, donguru_style, DEFAULT_PALETTE, FALLBACK_PALETTE, Palette};

/// This example's *own* help/error styling is fixed at compile time via
/// `#[command(styles = ...)]` (unlike `Cli::parse_styled()`, which picks a palette at runtime).
/// The theme(s) rendered *by* the example below are unaffected by this.
#[derive(Debug, Parser)]
#[command(
    name = "theme_preview",
    about = "Show the Donguru CLI colour theme(s) slot by slot",
    styles = donguru_style()
)]
struct Args {
    /// Which theme variant(s) to display
    #[arg(short, long, value_enum, default_value_t = ThemeChoice::All)]
    theme: ThemeChoice,
}

/// Selectable theme variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ThemeChoice {
    /// Whatever the CLI would pick for this terminal
    Auto,
    /// The 24-bit "Acorn Circuit" theme ([`DEFAULT_PALETTE`])
    Donguru,
    /// The 16-colour variant for terminals without truecolor ([`FALLBACK_PALETTE`])
    Fallback,
    /// clap's built-in default styling
    Default,
    /// No styling at all
    Plain,
    /// All of the above, side by side
    All,
}

/// A resolved variant: a label for the report, the styles themselves and pallet it was derived from.
struct Variant {
    label: String,
    styles: Styles,
    palette: Option<&'static Palette>,
}

impl Variant {
    /// A variant backed by one of the Donguru palettes.
    fn from_palette(label: &str, palette: &'static Palette) -> Self {
        Self {
            label: label.to_owned(),
            styles: palette.into(),
            palette: Some(palette),
        }
    }
}

impl ThemeChoice {
    /// Expand the selection into the concrete variants to render.
    fn resolve(self) -> Vec<Variant> {
        match self {
            // `select_style()` is not introspectable, so name it by comparing against
            // the styles built from the known truecolor palette.
            Self::Auto => {
                let styles = theme::select_style();
                let (detected, palette) = if styles_eq(&styles, &(&DEFAULT_PALETTE).into()) {
                    ("auto (donguru)", &DEFAULT_PALETTE)
                } else {
                    ("auto (fallback)", &FALLBACK_PALETTE)
                };
                vec![Variant {
                    label: detected.to_owned(),
                    styles,
                    palette: Some(palette),
                }]
            }
            Self::Donguru => vec![Variant::from_palette("donguru", &DEFAULT_PALETTE)],
            Self::Fallback => vec![Variant::from_palette("fallback", &FALLBACK_PALETTE)],
            Self::Default => vec![Variant {
                label: "default".to_owned(),
                styles: Styles::styled(),
                palette: None,
            }],
            Self::Plain => vec![Variant {
                label: "plain".to_owned(),
                styles: Styles::plain(),
                palette: None,
            }],
            Self::All => [Self::Donguru, Self::Fallback, Self::Default, Self::Plain]
                .into_iter()
                .flat_map(Self::resolve)
                .collect(),
        }
    }
}

/// One row of the overview: slot name, what clap uses it for, and how to read it.
struct Slot {
    name: &'static str,
    used_for: &'static str,
    pick: fn(&Styles) -> &Style,
}

const SLOTS: &[Slot] = &[
    Slot {
        name: "header",
        used_for: "section titles: Options:, Commands: (NOT Usage:)",
        pick: Styles::get_header,
    },
    Slot {
        name: "usage",
        used_for: "the \"Usage:\" label itself, in help and errors",
        pick: Styles::get_usage,
    },
    Slot {
        name: "literal",
        used_for: "things typed verbatim: flags, subcommands",
        pick: Styles::get_literal,
    },
    Slot {
        name: "placeholder",
        used_for: "value slots: <SELECTOR>, [OPTIONS]",
        pick: Styles::get_placeholder,
    },
    Slot {
        name: "valid",
        used_for: "suggestions, incl. the \"tip:\" label",
        pick: Styles::get_valid,
    },
    Slot {
        name: "invalid",
        used_for: "the offending input in an error",
        pick: Styles::get_invalid,
    },
    Slot {
        name: "error",
        used_for: "the \"error:\" prefix",
        pick: Styles::get_error,
    },
    Slot {
        name: "context",
        used_for: "extra error context (from Styles::styled())",
        pick: Styles::get_context,
    },
    Slot {
        name: "context_value",
        used_for: "values inside error context (from Styles::styled())",
        pick: Styles::get_context_value,
    },
];

/// The color slots a [`Palette`] actually defines, the remaining `Styles` slots are
/// inherited from `Styles::styled()`.
const PALETTE_SLOTS: &[(&str, fn(&Palette) -> Color)] = &[
    ("header", Palette::header),
    ("usage", Palette::usage),
    ("literal", Palette::literal),
    ("placeholder", Palette::placeholder),
    ("valid", Palette::valid),
    ("invalid", Palette::invalid),
    ("error", Palette::error),
];

/// Width of one rendered style cell in the comparison table.
const CELL: usize = 22;

/// The styles this preview uses for its own chrome, taken from the Donguru palette.
struct Chrome {
    header: Style,
    literal: Style,
    placeholder: Style,
    valid: Style,
    invalid: Style,
}

impl Chrome {
    fn new() -> Self {
        let styles: Styles = (&DEFAULT_PALETTE).into();
        Self {
            header: *styles.get_header(),
            literal: *styles.get_literal(),
            placeholder: *styles.get_placeholder(),
            valid: *styles.get_valid(),
            invalid: *styles.get_invalid(),
        }
    }
}

fn main() {
    // Keep colour on even when piped; this tool exists to show colour.
    if std::env::var_os("NO_COLOR").is_none() {
        // SAFETY: single-threaded, before any output or further env access.
        unsafe { std::env::set_var("CLICOLOR_FORCE", "1") };
    }

    let args = Args::parse();
    let variants = args.theme.resolve();
    let chrome = Chrome::new();
    let mut out = anstream::stdout().lock();

    banner(&mut out, &chrome);
    detection(&mut out, &chrome, &variants);
    slot_table(&mut out, &chrome, &variants);
    palette_table(&mut out, &chrome, &variants);
    slot_legend(&mut out, &chrome);
    effect_legend(&mut out, &chrome);
    footer(&mut out, &chrome);
}

fn banner(out: &mut impl Write, chrome: &Chrome) {
    let Chrome {
        header,
        literal,
        placeholder,
        ..
    } = chrome;

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  {literal}donguru{Reset} {placeholder}ドングル{Reset}  {header}Acorn Circuit theme{Reset}"
    );
    let _ = writeln!(out);
}

/// Show what the runtime detection in `theme::select_style()` decided, and why.
fn detection(out: &mut impl Write, chrome: &Chrome, variants: &[Variant]) {
    let Chrome {
        header,
        literal,
        placeholder,
        valid,
        invalid,
    } = chrome;

    let colorterm = std::env::var("COLORTERM").unwrap_or_else(|_| "<unset>".into());
    let term = std::env::var("TERM").unwrap_or_else(|_| "<unset>".into());

    let truecolor = styles_eq(&theme::select_style(), &(&DEFAULT_PALETTE).into());
    let (marker, label) = if truecolor {
        (valid, "truecolor -> DEFAULT_PALETTE (24-bit)")
    } else {
        (invalid, "no truecolor -> FALLBACK_PALETTE (16 colours)")
    };

    let shown = variants
        .iter()
        .map(|v| v.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let _ = writeln!(out, "{header}Runtime detection{Reset}");
    let _ = writeln!(
        out,
        "  {literal}COLORTERM{Reset} = {placeholder}{colorterm}{Reset}"
    );
    let _ = writeln!(
        out,
        "  {literal}TERM{Reset}      = {placeholder}{term}{Reset}"
    );
    let _ = writeln!(out, "  {marker}{label}{Reset}");
    let _ = writeln!(
        out,
        "  {literal}showing:{Reset}  {placeholder}{shown}{Reset}"
    );
    let _ = writeln!(out);
}

/// Table of every slot, one column per selected variant.
fn slot_table(out: &mut impl Write, chrome: &Chrome, variants: &[Variant]) {
    let Chrome {
        header,
        placeholder,
        ..
    } = chrome;

    let _ = writeln!(out, "{header}Theme slots{Reset}");
    let _ = writeln!(
        out,
        "{}",
        table_head(placeholder, "slot", variants.iter())
    );

    for slot in SLOTS {
        let mut row = format!("  {:<15}", slot.name);
        for variant in variants {
            let style = (slot.pick)(&variant.styles);
            // Pad before styling: ANSI escapes would break `{:<}` width maths.
            row.push_str(&format!("  {style}{:<CELL$}{Reset}", describe(style)));
        }
        let _ = writeln!(out, "{row}");
    }
    let _ = writeln!(out);
}

/// The raw palette colours behind the styles, without the bold/underline effects.
fn palette_table(out: &mut impl Write, chrome: &Chrome, variants: &[Variant]) {
    let Chrome {
        header,
        placeholder,
        ..
    } = chrome;

    // Only palette-backed variants have colours to show.
    let variants: Vec<&Variant> = variants.iter().filter(|v| v.palette.is_some()).collect();
    if variants.is_empty() {
        return;
    }

    let _ = writeln!(out, "{header}Palette colours{Reset}");
    let _ = writeln!(
        out,
        "{}",
        table_head(placeholder, "colour", variants.iter().copied())
    );

    for (name, pick) in PALETTE_SLOTS {
        let mut row = format!("  {:<15}", name);
        for variant in &variants {
            let Some(palette) = variant.palette else {
                continue;
            };
            let color = pick(palette);
            let swatch = Style::new().fg_color(Some(color));
            row.push_str(&format!(
                "  {swatch}{:<CELL$}{Reset}",
                format!("██ {}", color_name(&color))
            ));
        }
        let _ = writeln!(out, "{row}");
    }
    let _ = writeln!(out);
}

/// Shared header row for the comparison tables.
fn table_head<'a>(
    placeholder: &Style,
    first_column: &str,
    variants: impl Iterator<Item = &'a Variant>,
) -> String {
    let mut head = format!("  {placeholder}{first_column:<15}");
    for variant in variants {
        head.push_str(&format!("  {:<CELL$}", variant.label));
    }
    head.push_str(&format!("{Reset}"));
    head
}

/// What each slot is used for (kept out of the table so it stays narrow).
fn slot_legend(out: &mut impl Write, chrome: &Chrome) {
    let Chrome {
        header,
        literal,
        placeholder,
        ..
    } = chrome;

    let _ = writeln!(out, "{header}What each slot styles{Reset}");
    for slot in SLOTS {
        let _ = writeln!(
            out,
            "  {literal}{:<15}{Reset} {placeholder}{}{Reset}",
            slot.name, slot.used_for
        );
    }
    let _ = writeln!(out);
}

/// Show each effect in isolation so bold/underline are distinguishable.
fn effect_legend(out: &mut impl Write, chrome: &Chrome) {
    let Chrome {
        header,
        placeholder,
        ..
    } = chrome;

    let _ = writeln!(out, "{header}Effects{Reset}");
    for (name, style) in [
        ("plain", Style::new()),
        ("bold", Style::new().bold()),
        ("underline", Style::new().underline()),
        ("bold+underline", Style::new().bold().underline()),
        ("dimmed", Style::new().dimmed()),
    ] {
        let _ = writeln!(
            out,
            "  {placeholder}{name:<15}{Reset} {style}The quick brown dongle{Reset}"
        );
    }
    let _ = writeln!(out);
}

fn footer(out: &mut impl Write, chrome: &Chrome) {
    let placeholder = &chrome.placeholder;
    let _ = writeln!(
        out,
        "{placeholder}Variants: --theme {{auto|donguru|fallback|default|plain|all}}.{Reset}"
    );
    let _ = writeln!(
        out,
        "{placeholder}See the theme in real help output: donguru --help. Disable colour: NO_COLOR=1.{Reset}"
    );
    let _ = writeln!(out);
}

/// Human-readable summary of a style: colour plus active effects.
fn describe(style: &Style) -> String {
    let color = match style.get_fg_color() {
        Some(c) => color_name(&c),
        None => "default".into(),
    };
    let effects = style.get_effects();
    if effects.is_plain() {
        color
    } else {
        // Debug prints `Effects(BOLD | UNDERLINE)`; keep just the inner part.
        let dbg = format!("{effects:?}");
        let inner = dbg
            .trim_start_matches("Effects(")
            .trim_end_matches(')')
            .to_ascii_lowercase()
            .replace(" | ", "+");
        format!("{color} {inner}")
    }
}

fn color_name(color: &Color) -> String {
    match color {
        Color::Rgb(rgb) => format!("#{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2),
        Color::Ansi(ansi) => format!("{ansi:?}").to_ascii_lowercase(),
        Color::Ansi256(c) => format!("ansi256({})", c.0),
    }
}

/// `Styles` has no `PartialEq`, so compare the slots we care about.
fn styles_eq(a: &Styles, b: &Styles) -> bool {
    SLOTS.iter().all(|slot| (slot.pick)(a) == (slot.pick)(b))
}
