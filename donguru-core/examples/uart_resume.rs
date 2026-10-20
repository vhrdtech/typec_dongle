//! Hardware check of credit sink resume (WireWeaver TEST-26, API-34) on the header UART, `uart[1]`: sends numbered
//! lines on its `tx` credit sink while receiving them back on its `rx` stream (a jumper between header pins 1 and 2,
//! IO7 and IO8), drops the link in the middle of the transfer, and checks that every byte went out on the line
//! exactly once and in order.
//!
//! The line is a stream of 32-byte lines (`0000042 <hash>...\n`), so the byte at any offset is known. Everything the
//! device receives is compared with that at the position the device gives it: a byte sent twice or not at all shifts
//! every position after it and shows as a mismatch. Bytes the device could not hand to the host (it was away) come as
//! gaps with an exact count, and are reported.
//!
//! ```sh
//! cargo run -p donguru-core --example uart_resume --release -- --mode plain
//! cargo run -p donguru-core --example uart_resume --release -- --mode reconnect --drops 3
//! cargo run -p donguru-core --example uart_resume --release -- --mode restart
//! ```
//!
//! * `plain`: no drop, the baseline rate.
//! * `reconnect`: `--drops` times, `disconnect().keep_streams()` right after a send (bytes unacknowledged), wait
//!   `--down-ms`, `Commander::reconnect()`; the same sink goes on.
//! * `restart`: sends up to half, saves the sink's cursor and the receiver's state into `--state` and exits without
//!   closing anything; a second process of this program (started by this one) continues with `uart_tx_resume(cursor)`.

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use donguru_core::mock::MockDevice;
use donguru_core::{Dongle, Selector};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wire_weaver::shrink_wrap::{UVlq32, UVlq64};
use wire_weaver_client::{StreamCursor, StreamEvent};

/// Bytes of one numbered line, the newline included.
const LINE: usize = 32;
/// Header UART of `uart[]`.
const HEADER: u32 = 1;
/// Largest item of the `tx` sink, `ww_uart::UART_TX_MAX`.
const PIECE: usize = 256;

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Mode {
    Plain,
    Reconnect,
    Restart,
}

#[derive(Parser)]
#[command(about = "Credit sink resume on the Donguru header UART (TEST-26)")]
struct Args {
    /// Which dongle (as `dg --device`), when more than one is connected
    #[arg(long)]
    device: Option<Selector>,
    #[arg(long, value_enum, default_value = "reconnect")]
    mode: Mode,
    /// Bytes to send, a multiple of 32 (numbered lines)
    #[arg(long, default_value_t = 1_000_000)]
    bytes: u64,
    /// `reconnect`: how many times the link is dropped, evenly spread over the transfer
    #[arg(long, default_value_t = 3)]
    drops: u64,
    /// `reconnect`: how long the link stays down
    #[arg(long, default_value_t = 200)]
    down_ms: u64,
    /// `restart`: where the first process leaves the cursors for the second
    #[arg(long, default_value = "uart_resume_state.json")]
    state: PathBuf,
    /// `restart`: this is the second process, continue from `--state`
    #[arg(long, hide = true)]
    resume: bool,
    /// Run against the in-process mock instead of a dongle (`plain` and `reconnect`): a dry run of this program
    #[arg(long)]
    mock: bool,
    /// How long to wait for the line to deliver the last bytes
    #[arg(long, default_value_t = 5)]
    drain_secs: u64,
}

/// What the first process of `restart` hands over.
#[derive(Serialize, Deserialize)]
struct State {
    tx_session: u32,
    tx_pos: u64,
    rx_session: u32,
    rx_pos: u64,
    rx: RxStats,
    total: u64,
}

/// What the receiver saw, checked against the numbered lines.
#[derive(Default, Clone, Serialize, Deserialize)]
struct RxStats {
    /// Position after the last byte (or gap) seen: the line is accounted for up to here.
    end: u64,
    received: u64,
    /// Bytes the device could not give the host, with the count the device gave.
    lost: u64,
    gaps: u64,
    /// Items whose bytes are not the numbered lines at their position.
    mismatches: u64,
    first_mismatch: Option<String>,
    resumed: u64,
    /// Positions went back to 0 (the device's receiver started a new session).
    restarted: u64,
}

/// The numbered lines, the bytes `at..at + len` of the line.
fn expected(at: u64, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut pos = at;
    while out.len() < len {
        let n = pos / LINE as u64;
        let line = format!(
            "{n:07} {:016x}-------\n",
            n.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        );
        debug_assert_eq!(line.len(), LINE);
        let skip = (pos % LINE as u64) as usize;
        let take = (LINE - skip).min(len - out.len());
        out.extend_from_slice(&line.as_bytes()[skip..skip + take]);
        pos += take as u64;
    }
    out
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.bytes == 0 || args.bytes % LINE as u64 != 0 {
        bail!("--bytes must be a multiple of {LINE}");
    }
    if args.mock && args.mode == Mode::Restart {
        bail!("--mock has no second process to restart into");
    }
    if args.mode == Mode::Restart && !args.resume {
        return restart_first(&args).await;
    }
    run(&args).await
}

/// The first process of `restart`: sends up to half and leaves, then runs the second one and reports for both.
async fn restart_first(args: &Args) -> Result<()> {
    let half = args.bytes / 2 / LINE as u64 * LINE as u64;
    let dongle = Dongle::open(args.device.as_ref()).await?;
    let uart = dongle.client().uart(HEADER);
    let rx_stats = Arc::new(Mutex::new(RxStats::default()));
    let mut rx = uart.rx().call().await.context("opening uart[1].rx")?;
    let rx_task = {
        let stats = rx_stats.clone();
        tokio::spawn(async move {
            let _ = receive(&mut rx, &stats).await;
            rx
        })
    };
    let mut tx = uart.tx().await.context("opening uart[1].tx")?;
    let started = Instant::now();
    send_range(&mut tx, 0, half).await?;
    // not flushed: what the device has not acknowledged yet is what the second process has to find out
    let cursor = tx.cursor().context("no session")?;
    println!(
        "process 1: sent {half} bytes in {:.2} s, device had accepted {} of them (cursor)",
        started.elapsed().as_secs_f64(),
        cursor.pos.0
    );
    // saved when the receiver has seen what the device sent so far; the device's session of rx is what to resume
    let rx_session = status_session(&dongle).await?;
    let rx_cursor = {
        let stats = rx_stats.lock().unwrap();
        (stats.end, stats.clone())
    };
    let state = State {
        tx_session: cursor.session.0,
        tx_pos: cursor.pos.0,
        rx_session,
        rx_pos: rx_cursor.0,
        rx: rx_cursor.1,
        total: args.bytes,
    };
    std::fs::write(&args.state, serde_json::to_vec_pretty(&state)?)?;
    // the process "dies": the link goes, nothing is flushed or closed (a sink sends nothing when dropped)
    rx_task.abort();
    drop(tx);
    drop(dongle);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let status = tokio::process::Command::new(std::env::current_exe()?)
        .args(std::env::args_os().skip(1))
        .arg("--resume")
        .status()
        .await?;
    if !status.success() {
        bail!("the second process failed: {status}");
    }
    Ok(())
}

/// The session of the device's receiver, to resume it.
async fn status_session(dongle: &Dongle) -> Result<u32> {
    let status = dongle.client().uart(HEADER).rx_status().read().await?;
    status.session.map(|s| s.0).context("rx has no session")
}

async fn run(args: &Args) -> Result<()> {
    let total = args.bytes;
    let mock = args.mock.then(|| MockDevice::start("uart_resume"));
    let dongle = match &mock {
        Some(mock) => mock.open().await?,
        None => Dongle::open(args.device.as_ref()).await?,
    };
    let uart = dongle.client().uart(HEADER);
    let rx_stats = Arc::new(Mutex::new(RxStats::default()));

    let (mut tx, mut rx, mut at) = if args.resume {
        let state: State = serde_json::from_slice(&std::fs::read(&args.state)?)?;
        *rx_stats.lock().unwrap() = state.rx;
        let rx_cursor = StreamCursor {
            session: UVlq32(state.rx_session),
            pos: UVlq64(state.rx_pos),
        };
        let rx = uart.rx_resume(rx_cursor).call().await?;
        let cursor = StreamCursor {
            session: UVlq32(state.tx_session),
            pos: UVlq64(state.tx_pos),
        };
        let t = Instant::now();
        let (tx, at) = uart
            .tx_resume(cursor)
            .await
            .context("resuming uart[1].tx")?;
        println!(
            "process 2: resumed at {at} (saved cursor {}, the device had {} more) in {:.1} ms",
            state.tx_pos,
            at - state.tx_pos,
            t.elapsed().as_secs_f64() * 1000.0
        );
        (tx, rx, at)
    } else {
        let rx = uart.rx().call().await.context("opening uart[1].rx")?;
        let tx = uart.tx().await.context("opening uart[1].tx")?;
        (tx, rx, 0)
    };
    let rx_task = {
        let stats = rx_stats.clone();
        tokio::spawn(async move {
            let _ = receive(&mut rx, &stats).await;
        })
    };

    let started = Instant::now();
    let drop_every = (total / (args.drops + 1)).max(PIECE as u64) / LINE as u64 * LINE as u64;
    let mut next_drop = if args.mode == Mode::Reconnect && args.drops > 0 {
        drop_every
    } else {
        u64::MAX
    };
    let mut resumes = vec![];
    while at < total {
        let end = (at + PIECE as u64).min(total);
        let piece = expected(at, (end - at) as usize);
        tx.send_bytes(&piece).await.context("send")?;
        at = end;
        if at >= next_drop {
            next_drop += drop_every;
            let t = Instant::now();
            dongle.client().cmd.disconnect().keep_streams().await?;
            tokio::time::sleep(Duration::from_millis(args.down_ms)).await;
            let t_down = t.elapsed();
            let t = Instant::now();
            let resumed = dongle.client().cmd.reconnect().await.context("reconnect")?;
            let t_reconnect = t.elapsed();
            // the first byte accepted after the resume
            let t = Instant::now();
            let progress = tx.credit_progress();
            tx.flush().await.context("flush after reconnect")?;
            resumes.push((at, resumed, t_down, t_reconnect, t.elapsed(), progress));
        }
    }
    tx.flush().await.context("final flush")?;
    let sent_in = started.elapsed();

    // let the line deliver the rest
    let deadline = Instant::now() + Duration::from_secs(args.drain_secs);
    loop {
        let end = rx_stats.lock().unwrap().end;
        if end >= total || Instant::now() > deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    rx_task.abort();

    let stats = rx_stats.lock().unwrap().clone();
    let rate = total as f64 / sent_in.as_secs_f64();
    println!(
        "sent {total} bytes (up to offset {at}) in {:.2} s: {:.0} B/s ({:.0} baud-equivalent)",
        sent_in.as_secs_f64(),
        rate,
        rate * 10.0
    );
    for (i, (at, resumed, down, reconnect, first, progress)) in resumes.iter().enumerate() {
        println!(
            "drop {}: at offset {at}, link down {:.0} ms, reconnect() {:.1} ms ({resumed} resumed), flush after it {:.1} ms, progress before {progress:?}",
            i + 1,
            down.as_secs_f64() * 1000.0,
            reconnect.as_secs_f64() * 1000.0,
            first.as_secs_f64() * 1000.0,
        );
    }
    println!(
        "received: {} bytes in order, {} bytes lost by the device while no host was there ({} gaps), line accounted for up to {} of {total}",
        stats.received, stats.lost, stats.gaps, stats.end
    );
    println!(
        "mismatches: {} {}  resumes of rx: {}  rx restarts: {}",
        stats.mismatches,
        stats.first_mismatch.as_deref().unwrap_or(""),
        stats.resumed,
        stats.restarted
    );
    let ok = stats.mismatches == 0
        && stats.end == total
        && stats.received + stats.lost == total
        && stats.restarted == 0;
    if args.mode == Mode::Plain && stats.lost != 0 {
        bail!("FAIL: bytes lost with no drop");
    }
    if stats.end == 0 {
        bail!("FAIL: nothing came back on rx, is there a jumper between header pins 1 and 2?");
    }
    if !ok {
        bail!("FAIL: the line does not carry the numbered lines exactly once and in order");
    }
    println!("OK: every byte went out on the line exactly once, in order");
    Ok(())
}

/// Sends the numbered lines `from..to`.
async fn send_range(
    tx: &mut wire_weaver_client::Sink<wire_weaver::shrink_wrap::tail_bytes::TailBytesOwned>,
    from: u64,
    to: u64,
) -> Result<()> {
    let mut at = from;
    while at < to {
        let end = (at + PIECE as u64).min(to);
        tx.send_bytes(&expected(at, (end - at) as usize)).await?;
        at = end;
    }
    Ok(())
}

/// Compares what the device receives with the numbered lines at the position it names.
async fn receive(
    rx: &mut wire_weaver_client::PositionedStream<ww_uart::RxChunkOwned>,
    stats: &Mutex<RxStats>,
) -> Result<()> {
    loop {
        let event = rx.recv_event().await?;
        let mut s = stats.lock().unwrap();
        match event {
            StreamEvent::Item {
                pos, units, value, ..
            } => {
                let bytes = value.bytes.into_inner();
                if pos != s.end {
                    // a head's `lost` arrives as a Gap event first, so a jump is a position that was not accounted for
                    s.mismatches += 1;
                    let why = format!("item at {pos}, expected {}", s.end);
                    s.first_mismatch.get_or_insert(why);
                }
                let want = expected(pos, bytes.len());
                if bytes != want || units != bytes.len() as u64 {
                    s.mismatches += 1;
                    let at = bytes.iter().zip(&want).position(|(a, b)| a != b).unwrap_or(0);
                    let from = at.saturating_sub(4);
                    let to = (at + 12).min(bytes.len());
                    s.first_mismatch.get_or_insert(format!(
                        "item at {pos} ({} bytes, {units} units), first difference at +{at}: got {:?} want {:?}",
                        bytes.len(),
                        String::from_utf8_lossy(&bytes[from..to]),
                        String::from_utf8_lossy(&want[from..to.min(want.len())])
                    ));
                }
                s.received += bytes.len() as u64;
                s.end = pos + units;
            }
            StreamEvent::Gap { at, units, .. } => {
                if at != s.end {
                    s.mismatches += 1;
                    let why = format!("gap at {at}, expected {}", s.end);
                    s.first_mismatch.get_or_insert(why);
                }
                s.lost += units;
                s.gaps += 1;
                s.end = at + units;
            }
            StreamEvent::Resumed { ended, .. } => {
                s.resumed += 1;
                if ended.is_some() {
                    s.restarted += 1;
                    s.end = 0;
                }
            }
            StreamEvent::Closed => return Ok(()),
            _ => {}
        }
    }
}
