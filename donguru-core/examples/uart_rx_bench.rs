//! Hardware bench of the header UART's receive path at full line rate (FW-42): the dongle replays a text capture
//! from its flash out of `uart[1]`'s TX (`debug.uart_replay`, firmware feature `uart-replay`) into its own RX (a
//! jumper between header pins 1 and 2, IO7 and IO8), and this program receives `uart[1].rx` with and without
//! WireWeaver's stream compression (API-55).
//!
//! Per capture and mode it prints the UART bytes delivered per second, what crossed USB and the ratio, the bytes
//! lost (by the device, on the link, undecodable) and the CPU load from the firmware's `idle_us` counter (FW-43).
//! Every byte received is checked against the capture at its position, so a decoding fault shows as a mismatch.
//! Without compression the host offers no codec, as a host from before API-55 does: the device sends the chunks
//! as they are.
//!
//! ```sh
//! cargo run -p donguru-core --example uart_rx_bench --release -- \
//!     --elf fw/donguru/target/thumbv6m-none-eabi/release/donguru
//! cargo run -p donguru-core --example uart_rx_bench --release -- --capture top --mode lz4 --secs 10
//! ```

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use donguru_api::ReplayCapture;
use donguru_core::levels::Side;
use donguru_core::{Dongle, Selector};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wire_weaver_client::StreamEvent;
use ww_si::{Baud, Prefix};
use ww_uart::{Mode as LineMode, UartConfig};

/// Header UART of `uart[]`.
const HEADER: u32 = 1;

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Capture {
    Boot,
    Ls,
    Top,
    All,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// No codec offered (as a host from before API-55): chunks as they are
    Raw,
    /// LZ4 offered
    Lz4,
    Both,
}

#[derive(Parser)]
#[command(
    about = "Header UART receive path at full line rate, with and without compression (FW-42)"
)]
struct Args {
    /// Which dongle (as `dg --device`), when more than one is connected
    #[arg(long)]
    device: Option<Selector>,
    /// Line rate, 8N1; the dongle sets the nearest it can (6 Mbaud gives 5.818)
    #[arg(long, default_value_t = 6_000_000)]
    baud: u32,
    #[arg(long, value_enum, default_value = "all")]
    capture: Capture,
    #[arg(long, value_enum, default_value = "both")]
    mode: Mode,
    /// Seconds measured per capture and mode, after a warm-up of half a second
    #[arg(long, default_value_t = 5.0)]
    secs: f64,
    /// The firmware's ELF, to decode its counters (else found by build id, `cnt elf add`)
    #[arg(long)]
    elf: Option<PathBuf>,
}

/// The captures the firmware replays (`fw/donguru/replay/`).
fn capture_bytes(c: ReplayCapture) -> &'static [u8] {
    match c {
        ReplayCapture::Boot => include_bytes!("../../fw/donguru/replay/boot.txt"),
        ReplayCapture::Ls => include_bytes!("../../fw/donguru/replay/ls.txt"),
        ReplayCapture::Top => include_bytes!("../../fw/donguru/replay/top.txt"),
        _ => &[],
    }
}

/// What one run saw.
#[derive(Default)]
struct Run {
    /// UART bytes received in the measured window.
    bytes: u64,
    /// Compressed bytes (as on the link) and what they decoded to, in the window.
    link_in: u64,
    link_out: u64,
    lost_device: u64,
    lost_link: u64,
    lost_codec: u64,
    lost_host: u64,
    /// Bytes that are not the capture at their position, and where the first one was.
    mismatches: u64,
    first_mismatch: Option<u64>,
    secs: f64,
    idle: Option<f64>,
    counters: Vec<(String, u64)>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let dongle = Dongle::open(args.device.as_ref()).await?;
    let asked = UartConfig {
        baud_rate: Baud::new(Prefix::Unit, args.baud),
        data_bits: ww_uart::DataBits::Eight,
        parity: ww_uart::Parity::None,
        stop_bits: ww_uart::StopBits::Stop1,
        mode: LineMode::Asynchronous,
        prevent_back_feed: false,
        io_level: None,
        idle_chars: ww_uart::IDLE_CHARS_DEFAULT,
    };
    let got = dongle.uart(Side::Header).configure(asked).await?;
    let baud = got.baud_rate.to_f64();
    println!(
        "line {:.0} Bd: {:.1} kB/s of UART bytes at most",
        baud,
        baud / 10.0 / 1000.0
    );

    let captures: &[ReplayCapture] = match args.capture {
        Capture::Boot => &[ReplayCapture::Boot],
        Capture::Ls => &[ReplayCapture::Ls],
        Capture::Top => &[ReplayCapture::Top],
        Capture::All => &[ReplayCapture::Boot, ReplayCapture::Ls, ReplayCapture::Top],
    };
    let modes: &[bool] = match args.mode {
        Mode::Raw => &[false],
        Mode::Lz4 => &[true],
        Mode::Both => &[false, true],
    };
    println!(
        "{:<5} {:<4} {:>9} {:>9} {:>6} {:>6} {:>9} {:>6}  counters",
        "data", "mode", "UART kB/s", "USB kB/s", "ratio", "CPU %", "lost B", "bad B"
    );
    let mut failed = false;
    for &capture in captures {
        for &lz4 in modes {
            let run = bench(&dongle, &args, capture, lz4).await?;
            let lost = run.lost_device + run.lost_link + run.lost_codec + run.lost_host;
            let usb = if lz4 { run.link_in } else { run.bytes };
            let ratio = if lz4 && run.link_in > 0 {
                run.link_out as f64 / run.link_in as f64
            } else {
                1.0
            };
            let cpu = run
                .idle
                .map(|idle| format!("{:.0}", (1.0 - idle) * 100.0))
                .unwrap_or_else(|| "?".into());
            let counters: Vec<String> = run
                .counters
                .iter()
                .map(|(n, v)| format!("{n} +{v}"))
                .collect();
            println!(
                "{:<5} {:<4} {:>9.1} {:>9.1} {:>6.2} {:>6} {:>9} {:>6}  {}",
                format!("{capture:?}").to_lowercase(),
                if lz4 { "lz4" } else { "raw" },
                run.bytes as f64 / run.secs / 1000.0,
                usb as f64 / run.secs / 1000.0,
                ratio,
                cpu,
                lost,
                run.mismatches,
                counters.join(", ")
            );
            if lost > 0 {
                println!(
                    "      lost: device {}, link {}, undecodable {}, host queue {}",
                    run.lost_device, run.lost_link, run.lost_codec, run.lost_host
                );
            }
            if let Some(at) = run.first_mismatch {
                println!("      first byte not the capture at stream position {at}");
                failed = true;
            }
            if run.bytes == 0 {
                bail!("nothing came back on rx: is there a jumper between header pins 1 and 2?");
            }
        }
    }
    if failed {
        bail!("FAIL: bytes received are not the capture");
    }
    Ok(())
}

/// Counters of the firmware: `idle_us` and the UART's.
async fn counters(dongle: &Dongle, elf: Option<&std::path::Path>) -> Option<Vec<(String, u64)>> {
    match dongle.device().counters(elf, false).await {
        Ok(report) => Some(
            report
                .counters
                .into_iter()
                .filter(|c| c.name.contains("idle_us") || c.name.contains("uart"))
                .map(|c| (c.name, c.value))
                .collect(),
        ),
        Err(e) => {
            eprintln!("counters: {e}");
            None
        }
    }
}

async fn bench(dongle: &Dongle, args: &Args, capture: ReplayCapture, lz4: bool) -> Result<Run> {
    let data = capture_bytes(capture);
    let uart = dongle.client().uart(HEADER);
    let mut rx = uart
        .rx()
        .compression(lz4)
        .call()
        .await
        .context("opening uart[1].rx")?;
    let len = dongle
        .client()
        .debug()
        .uart_replay(capture)
        .call()
        .await
        .context(
            "debug.uart_replay (a firmware built without `uart-replay` answers Unsupported)",
        )?;
    if len as usize != data.len() {
        bail!(
            "the dongle's capture is {len} bytes, this program's {}",
            data.len()
        );
    }

    let mut run = Run::default();
    // where the replay was when the stream started: found from the first item
    let mut offset: Option<usize> = None;
    let warm_up = Instant::now() + Duration::from_millis(500);
    let mut window: Option<(
        Instant,
        Option<Vec<(String, u64)>>,
        wire_weaver_client::StreamStats,
    )> = None;
    let mut window_bytes = 0u64;
    loop {
        let now = Instant::now();
        if window.is_none() && now >= warm_up {
            let before = counters(dongle, args.elf.as_deref()).await;
            window = Some((Instant::now(), before, rx.stats()));
            window_bytes = 0;
        }
        if let Some((start, _, _)) = &window
            && start.elapsed().as_secs_f64() >= args.secs
        {
            break;
        }
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv_event())
            .await
            .context("no event from uart[1].rx for 2 s")??;
        match event {
            StreamEvent::Item { pos, value, .. } => {
                let bytes = value.bytes.into_inner();
                let off = *offset.get_or_insert_with(|| find(data, &bytes, pos));
                for (i, b) in bytes.iter().enumerate() {
                    let at = pos + i as u64;
                    if data[(off + at as usize) % data.len()] != *b {
                        run.mismatches += 1;
                        run.first_mismatch.get_or_insert(at);
                    }
                }
                window_bytes += bytes.len() as u64;
            }
            StreamEvent::Closed | StreamEvent::Disconnected => bail!("uart[1].rx ended"),
            _ => {}
        }
    }
    let (start, before, stats0) = window.context("no window")?;
    let after = counters(dongle, args.elf.as_deref()).await;
    run.secs = start.elapsed().as_secs_f64();
    let stats = rx.stats();
    dongle
        .client()
        .debug()
        .uart_replay(ReplayCapture::Stop)
        .call()
        .await
        .context("debug.uart_replay(Stop)")?;
    drop(rx);

    run.bytes = window_bytes;
    run.link_in = stats.codec_bytes_in - stats0.codec_bytes_in;
    run.link_out = stats.codec_bytes_out - stats0.codec_bytes_out;
    run.lost_device = stats.lost_device - stats0.lost_device;
    run.lost_link = stats.lost_link - stats0.lost_link;
    run.lost_codec = stats.lost_codec - stats0.lost_codec;
    run.lost_host = stats.lost_host - stats0.lost_host;
    if let (Some(before), Some(after)) = (before, after) {
        let value = |list: &[(String, u64)], name: &str| {
            list.iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .unwrap_or(0)
        };
        for (name, v) in &after {
            let d = v - value(&before, name).min(*v);
            if name.contains("idle_us") {
                run.idle = Some(d as f64 / 1e6 / run.secs);
            } else if d > 0 {
                run.counters.push((name.clone(), d));
            }
        }
    }
    Ok(run)
}

/// Where in the (cyclic) capture the stream's position 0 is, from an item at `pos`; 0 when it isn't found (every
/// byte then shows as a mismatch).
fn find(data: &[u8], bytes: &[u8], pos: u64) -> usize {
    let probe = &bytes[..bytes.len().min(64)];
    let twice = [data, data].concat();
    match twice.windows(probe.len()).position(|w| w == probe) {
        Some(at) => (at + data.len() - (pos as usize % data.len())) % data.len(),
        None => 0,
    }
}
