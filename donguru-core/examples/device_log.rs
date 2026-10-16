//! Shows a dongle's defmt log (`device.log`, API-10) over USB, decoded with the firmware's ELF: the log from before
//! this program connected comes first (the dongle keeps it, also across a crash and reset), then live lines;
//! messages the dongle dropped show as one line with their exact number. A test tool until `dg device log` (CLI-19).
//!
//! ```sh
//! cargo run -p donguru-core --example device_log -- --elf fw/donguru/target/thumbv6m-none-eabi/debug/donguru
//! cargo run -p donguru-core --example device_log -- --elf ... --flood 1000   # overload the logger on purpose
//! cargo run -p donguru-core --example device_log -- --elf ... --flood 3 --no-read   # lines left in the store
//! ```

use anyhow::{Context, Result, bail};
use clap::Parser;
use defmt_decoder::{DecodeError, Table};
use donguru_core::{Dongle, Selector};
use object::Object;
use std::path::PathBuf;
use wire_weaver::shrink_wrap::UVlq64;
use wire_weaver_client::{StreamCursor, StreamEvent};

#[derive(Parser)]
#[command(about = "defmt log of a Donguru over USB, decoded with its ELF")]
struct Args {
    /// The firmware's ELF, with the defmt table
    #[arg(long)]
    elf: PathBuf,
    /// Use the ELF even when its build id differs from the dongle's
    #[arg(long)]
    force: bool,
    /// Which dongle (as `dg --device`), when more than one is connected
    #[arg(long)]
    device: Option<Selector>,
    /// Make the dongle log this many lines at once after connecting (`debug.log_flood`)
    #[arg(long)]
    flood: Option<u32>,
    /// Only make the dongle log (`--flood`) and exit: the lines stay in its store, e.g. to see them survive a crash
    #[arg(long, requires = "flood")]
    no_read: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let elf =
        std::fs::read(&args.elf).with_context(|| format!("reading {}", args.elf.display()))?;
    let elf_build_id = object::File::parse(&*elf)?
        .build_id()?
        .unwrap_or_default()
        .to_vec();
    let table = Table::parse(&elf)?.context("the ELF has no defmt table")?;

    let dongle = Dongle::open(args.device.as_ref()).await?;
    let device = dongle.client().device();
    let log = device.log();
    let info = log.read_DEFMT().read().await?;
    let build_id = info.build_id.into_inner();
    println!(
        "dongle: defmt {:?}, build id {}",
        info.encoding,
        hex(&build_id)
    );
    if build_id != elf_build_id {
        let msg = format!(
            "the ELF's build id is {}, not the dongle's",
            hex(&elf_build_id)
        );
        if !args.force {
            bail!("{msg} (--force to use it anyway)");
        }
        println!("warning: {msg}");
    }

    if args.no_read {
        dongle
            .client()
            .debug()
            .log_flood(args.flood.unwrap_or(0))
            .call()
            .await?;
        return Ok(());
    }

    // resume the session the dongle started at boot (or kept from before a reset), from its start
    let status = log.frames_status().read().await?;
    let session = status.session.context("the dongle has no log session")?;
    let cursor = StreamCursor {
        session,
        pos: UVlq64(0),
    };
    let mut frames = log.frames_resume(cursor).call().await?;
    if let Some(count) = args.flood {
        dongle.client().debug().log_flood(count).call().await?;
        // lost lines are reported with the next line the dongle logs: one more, once the flood has left the
        // logger's ring, so the count shows without waiting for the dongle's next log line
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        dongle.client().debug().log_flood(1).call().await?;
    }

    let mut decoder = table.new_stream_decoder();
    loop {
        let event = match frames.recv_event().await {
            Ok(event) => event,
            Err(e) => {
                println!("stream ended: {e}");
                return Ok(());
            }
        };
        match event {
            StreamEvent::Item { value, units, .. } => {
                decoder.received(&value.bytes.into_inner());
                let mut decoded = 0;
                loop {
                    match decoder.decode() {
                        Ok(frame) => {
                            println!("{}", frame.display(true));
                            decoded += 1;
                        }
                        Err(DecodeError::UnexpectedEof) => break,
                        Err(DecodeError::Malformed) => {
                            println!("error: malformed frame (wrong ELF?)");
                            break;
                        }
                    }
                }
                if decoded != units {
                    println!("warning: {decoded} frames decoded, the dongle sent {units}");
                }
            }
            StreamEvent::Gap { units, cause, .. } => {
                println!("--- {units} log messages lost ({cause:?}) ---");
            }
            _ => {}
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
