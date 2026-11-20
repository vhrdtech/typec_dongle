//! Shell completion with live values (CLI-3, CLI-49): `COMPLETE=<shell> donguru -- ARGS` asks the binary itself, so
//! device serials come from the dongles plugged in right now, not from a list frozen into a script.

use std::ffi::OsStr;
use std::io::Write;
use std::time::Duration;

use clap::builder::StyledStr;
use clap_complete::CompleteEnv;
use clap_complete::engine::{CompletionCandidate, PathCompleter, ValueCompleter};
use clap_complete::env::Shells;

/// The environment variable that switches the binary into completion mode.
const VAR: &str = "COMPLETE";

/// How long a completion may wait for USB enumeration: a tab press must never hang.
const ENUMERATE_TIMEOUT: Duration = Duration::from_secs(2);

/// Answers a completion request from the shell and exits, or returns at once when this is a normal run.
pub fn complete_if_asked() {
    CompleteEnv::with_factory(<crate::cli::Cli as clap::CommandFactory>::command)
        .var(VAR)
        .complete();
}

/// The registration script of `shell` for the command called `bin` (`donguru` or `dg`): the shell calls back into
/// the binary found on `PATH` under that name.
pub fn registration(shell: &str, bin: &str, out: &mut dyn Write) -> anyhow::Result<()> {
    let shells = Shells::builtins();
    let completer = shells
        .completer(shell)
        .ok_or_else(|| anyhow::anyhow!("no completions for shell {shell:?}"))?;
    completer.write_registration(VAR, bin, bin, bin, out)?;
    Ok(())
}

fn help(text: String) -> Option<StyledStr> {
    Some(text.into())
}

/// Connected dongles for `--device`: serial numbers with the label and USB path as help, and the USB paths.
pub fn devices() -> Vec<CompletionCandidate> {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return vec![];
    };
    let listed = rt.block_on(async {
        tokio::time::timeout(ENUMERATE_TIMEOUT, donguru_core::device::list()).await
    });
    let Ok(Ok(entries)) = listed else {
        return vec![];
    };
    let mut out = Vec::new();
    for e in entries {
        let label = if e.label.is_empty() {
            "no label".to_string()
        } else {
            format!("\"{}\"", e.label)
        };
        out.push(
            CompletionCandidate::new(e.serial.clone())
                .help(help(format!("{label} at USB {}", e.path))),
        );
        out.push(
            CompletionCandidate::new(e.path.clone())
                .help(help(format!("{label}, serial {}", e.serial))),
        );
    }
    out
}

/// Firmware ELF files: directories to walk into, files without an extension (cargo's output) or `.elf`.
pub fn elf_files(current: &OsStr) -> Vec<CompletionCandidate> {
    PathCompleter::file()
        .filter(|path| {
            path.is_dir()
                || path
                    .extension()
                    .is_none_or(|ext| ext.eq_ignore_ascii_case("elf"))
        })
        .complete(current)
}

/// UART framings as `dg uart configure` and `dg settings set uart_plug` take them.
pub fn framings() -> Vec<CompletionCandidate> {
    [
        ("8N1", "8 data bits, no parity, 1 stop bit (the usual)"),
        ("8E1", "8 data bits, even parity, 1 stop bit"),
        ("8O1", "8 data bits, odd parity, 1 stop bit"),
        ("8N2", "8 data bits, no parity, 2 stop bits"),
        ("7E1", "7 data bits, even parity, 1 stop bit"),
        ("7O1", "7 data bits, odd parity, 1 stop bit"),
    ]
    .into_iter()
    .map(|(v, h)| CompletionCandidate::new(v).help(help(h.into())))
    .collect()
}

/// MCU pins for `dg debug pin`: the net names of the B153A with their port pin and what they do; the ones the
/// firmware always refuses say so.
pub fn nets() -> Vec<CompletionCandidate> {
    donguru_core::nets::B153A
        .iter()
        .map(|n| {
            let refused = donguru_core::debug::refused(n.pin)
                .map(|r| format!(" (refused: {r})"))
                .unwrap_or_default();
            CompletionCandidate::new(n.name).help(help(format!(
                "{}: {}{refused}",
                donguru_core::nets::port_pin_name(n.pin),
                n.note
            )))
        })
        .collect()
}

/// Common baud rates.
pub fn bauds() -> Vec<CompletionCandidate> {
    [
        ("9600", "slow default of many bootloaders"),
        ("19200", ""),
        ("38400", ""),
        ("57600", ""),
        ("115200", "the usual console speed"),
        ("230400", ""),
        ("460800", ""),
        ("921600", ""),
        ("1000000", ""),
        ("2000000", ""),
        ("3000000", ""),
    ]
    .into_iter()
    .map(|(v, h)| {
        let c = CompletionCandidate::new(v);
        if h.is_empty() {
            c
        } else {
            c.help(help(h.into()))
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_calls_back_into_the_named_binary() {
        for shell in ["bash", "zsh", "fish"] {
            let mut out = Vec::new();
            registration(shell, "dg", &mut out).unwrap();
            let script = String::from_utf8(out).unwrap();
            assert!(script.contains("COMPLETE"), "{shell}: {script}");
            assert!(script.contains("dg"), "{shell}");
        }
        assert!(registration("nushell", "dg", &mut Vec::new()).is_err());
    }

    #[test]
    fn framings_and_bauds_have_values() {
        assert!(framings().iter().any(|c| c.get_value() == "8N1"));
        assert!(bauds().iter().any(|c| c.get_value() == "115200"));
        assert!(nets().iter().any(|c| c.get_value() == "RELAY_EN"));
    }
}
