//! The header UART, `uart[1]` (API-14): USART5 on PB3 (IO7, TX) and PB4 (IO8, RX), 8N1 at [BAUD] from boot until
//! the host sets another line with [configure], for `src/ww/uart.rs`.
//!
//! DMA does the moving; two tasks decouple it from the server task through pipes. `tx_task` writes what the `tx`
//! sink put into [TX] and counts it in [take_tx_done], which the server task gives back to the host as credit once
//! the bytes left the pipe and went out on the line. `rx_task` reads the receiver's DMA ring into [RX]; what does not
//! fit is counted in [take_rx_dropped] and the host is told how many bytes it missed.
//!
//! With the feature `uart-replay`, `tx_task` can instead send a text capture from flash in a loop ([replay]), a
//! source at the full line rate for the receive path with a jumper from IO7 to IO8 (FW-42).
//!
//! A new line setting ([configure]) waits until `tx_task` has sent what it took out of [TX] (bytes the host sent
//! before `configure` go out at the old rate); then `rx_task` stops the receiver, sets the USART up and starts it again.

use core::sync::atomic::Ordering;
use defmt::{unwrap, warn};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_stm32::mode::Async;
use embassy_stm32::usart::{Config, Uart, UartRx, UartTx};

/// What embassy-stm32 sets the USART up with.
pub type LineConfig = Config;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_sync::signal::Signal;
use portable_atomic::AtomicU32; // the M0+ has no atomic read-modify-write
use static_cell::StaticCell;

use crate::Irqs;

/// Line rate at boot, 8N1.
pub const BAUD: u32 = 921_600;
/// USART5's clock: PCLK, 64 MHz (`main.rs`). USART5 has no kernel clock mux on the G0B1.
const PCLK: u32 = 64_000_000;
/// Slowest rate: PCLK / 256 (the largest PRESC) / 0xFFFF (the largest BRR).
pub const MIN_BAUD: u32 = 4;
/// Fastest rate: PCLK / 8 with OVER8. Above 4 Mbaud (PCLK / 16) only PCLK / k is possible, OVER8 drops the
/// lowest bit of USARTDIV (RM0444 34.5.7): 8, 7.11, 6.4, 5.82, 5.33 Mbaud.
pub const MAX_BAUD: u32 = PCLK / 8;

/// Bytes the `tx` sink accepts ahead, its credit: the pipe [TX] holds all of them.
pub const TX_BUF: usize = 2048;
/// Received bytes held for the server task, which sends them as chunks: 28 ms at 5.8 Mbaud, for bursts above what
/// USB carries (~520 kB/s).
pub const RX_BUF: usize = 16384;
/// The receiver's DMA ring: 7 ms at 5.8 Mbaud (44 ms at [BAUD]); `rx_task` empties it at every half and idle line.
const RX_RING: usize = 4096;

static TX: Pipe<CriticalSectionRawMutex, TX_BUF> = Pipe::new();
static RX: Pipe<CriticalSectionRawMutex, RX_BUF> = Pipe::new();
/// Bytes of [TX] written out on the line since the server task looked.
static TX_DONE: AtomicU32 = AtomicU32::new(0);
/// Received bytes that did not fit in [RX] since the server task looked.
static RX_DROPPED: AtomicU32 = AtomicU32::new(0);
/// The server task has something to do: credit to give back, or received bytes to send.
static EVENT: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// A line setting for `tx_task`, which hands it on to `rx_task` through [APPLY] once its last write is out.
static CONFIGURE: Signal<CriticalSectionRawMutex, Config> = Signal::new();
static APPLY: Signal<CriticalSectionRawMutex, Config> = Signal::new();
/// `rx_task` has set the USART up, `tx_task` goes on.
static APPLIED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static RX_RING_BUF: StaticCell<[u8; RX_RING]> = StaticCell::new();
/// What `tx_task` replays from now on, empty for nothing ([replay]).
static REPLAY: Signal<CriticalSectionRawMutex, &'static [u8]> = Signal::new();

/// The text captures [replay] sends (`None` for one this firmware doesn't know), 32 KiB each (`replay/`, CR LF line ends as from a serial console).
#[cfg(feature = "uart-replay")]
pub fn capture(which: donguru_api::ReplayCapture) -> Option<&'static [u8]> {
    use donguru_api::ReplayCapture;
    Some(match which {
        ReplayCapture::Stop => &[],
        ReplayCapture::Boot => include_bytes!("../replay/boot.txt"),
        ReplayCapture::Ls => include_bytes!("../replay/ls.txt"),
        ReplayCapture::Top => include_bytes!("../replay/top.txt"),
        ReplayCapture::Unknown(_) => return None,
    })
}

/// Sends `data` on the line in a loop, by DMA straight from flash, until the next call; empty stops. Bytes of the
/// `tx` sink wait in [TX] meanwhile.
pub fn replay(data: &'static [u8]) {
    REPLAY.signal(data);
}

pub fn init(spawner: Spawner, r: crate::UartResources) {
    let mut config = Config::default();
    config.baudrate = BAUD;
    let uart = unwrap!(Uart::new(
        r.usart5, r.pb3, r.pb4, r.tx_dma, r.rx_dma, Irqs, config
    ));
    let (tx, rx) = uart.split();
    spawner.spawn(unwrap!(tx_task(tx)));
    spawner.spawn(unwrap!(rx_task(rx, RX_RING_BUF.init([0; RX_RING]))));
}

/// The rate the USART runs at when asked for `baud`, `None` outside [MIN_BAUD]..=[MAX_BAUD]. The same choice as
/// embassy-stm32's `find_and_set_brr`: the smallest prescaler whose divider fits, the divider rounded to the nearest,
/// OVER8 when it is below 16; [line_rate] reads back what was set.
pub fn rate_for(baud: u32) -> Option<u32> {
    if baud == 0 {
        return None;
    }
    for presc in PRESC {
        let clock = PCLK / presc;
        let div = clock / baud + ((clock % baud) + baud / 2) / baud;
        if div < 16 {
            // only with presc 1: the dividers shrink with it
            return (div >= 8).then(|| rounded_div(clock, div));
        }
        if div < 0x1_0000 {
            return Some(rounded_div(clock, div));
        }
    }
    None
}

/// The dividers of PRESC, by its code.
const PRESC: [u32; 12] = [1, 2, 4, 6, 8, 10, 12, 16, 32, 64, 128, 256];

fn rounded_div(a: u32, b: u32) -> u32 {
    (a + b / 2) / b
}

/// The rate the USART runs at now, from BRR, PRESC and OVER8.
fn line_rate() -> u32 {
    let r = embassy_stm32::pac::USART5;
    let brr = r.brr().read().0 & 0xFFFF;
    let div = if r.cr1().read().over8() == embassy_stm32::pac::usart::vals::Over8::Oversampling8 {
        // USARTDIV = BRR[15:4] << 4 | BRR[2:0] << 1, the rate 2 * PCLK / USARTDIV
        (brr >> 4) << 3 | (brr & 0x7)
    } else {
        brr
    };
    // codes from 11 up divide by 256 too
    let presc = PRESC[(r.presc().read().prescaler().to_bits() as usize).min(PRESC.len() - 1)];
    if div == 0 {
        return 0;
    }
    rounded_div(PCLK / presc, div)
}

/// Sets up the line with `config` once the bytes already taken for sending are out; [rate_for] says the rate.
pub fn configure(config: Config) {
    CONFIGURE.signal(config);
}

/// Writes all of `data` into `pipe` that has room, returns how many bytes fit. A pipe takes only the contiguous
/// space at the end of its ring per call, so a write that wraps needs two.
fn write_all<const N: usize>(pipe: &Pipe<CriticalSectionRawMutex, N>, mut data: &[u8]) -> usize {
    let mut written = 0;
    while !data.is_empty() {
        let Ok(n) = pipe.try_write(data) else { break };
        written += n;
        data = &data[n..];
    }
    written
}

/// Queues `data` for sending, returns how many bytes fit (all of them while the host keeps to its credit).
pub fn tx_write(data: &[u8]) -> usize {
    write_all(&TX, data)
}

/// Bytes sent on the line since the last call, to be given back to the host as credit.
pub fn take_tx_done() -> u32 {
    TX_DONE.swap(0, Ordering::Relaxed)
}

/// Received bytes lost since the last call.
pub fn take_rx_dropped() -> u32 {
    RX_DROPPED.swap(0, Ordering::Relaxed)
}

/// Moves received bytes into `buf`, returns how many.
pub fn read_rx(buf: &mut [u8]) -> usize {
    RX.try_read(buf).unwrap_or(0)
}

/// Completes when [take_tx_done], [take_rx_dropped] or [read_rx] have something.
pub async fn wait_event() {
    EVENT.wait().await
}

#[embassy_executor::task]
async fn tx_task(mut tx: UartTx<'static, Async>) {
    let mut buf = [0u8; ww_uart::UART_TX_MAX];
    let mut replaying: &'static [u8] = &[];
    loop {
        if !replaying.is_empty() {
            // the write in flight is dropped (its DMA stopped) for a new setting or capture
            match select3(CONFIGURE.wait(), REPLAY.wait(), tx.write(replaying)).await {
                Either3::First(config) => set_line(&mut tx, config).await,
                Either3::Second(data) => replaying = data,
                Either3::Third(Ok(())) => {}
                Either3::Third(Err(e)) => warn!("uart[1] replay: {:?}", e),
            }
            continue;
        }
        // a new setting first: what the host sent after `configure` is for the new line
        let n = match select3(CONFIGURE.wait(), REPLAY.wait(), TX.read(&mut buf)).await {
            Either3::First(config) => {
                set_line(&mut tx, config).await;
                continue;
            }
            Either3::Second(data) => {
                replaying = data;
                continue;
            }
            Either3::Third(n) => n,
        };
        if let Err(e) = tx.write(&buf[..n]).await {
            warn!("uart[1] tx: {:?}", e);
        }
        TX_DONE.fetch_add(n as u32, Ordering::Relaxed);
        EVENT.signal(());
    }
}

/// Waits until the last byte is out, then has `rx_task` set the line up with `config`.
async fn set_line(tx: &mut UartTx<'static, Async>, config: Config) {
    if let Err(e) = tx.flush().await {
        warn!("uart[1] flush: {:?}", e);
    }
    APPLY.signal(config);
    APPLIED.wait().await;
}

#[embassy_executor::task]
async fn rx_task(rx: UartRx<'static, Async>, ring: &'static mut [u8; RX_RING]) {
    let mut rx = rx.into_ring_buffered(ring);
    let mut buf = [0u8; 512];
    loop {
        let read = match select(APPLY.wait(), rx.read(&mut buf)).await {
            Either::First(config) => {
                // what is still in the DMA ring goes: it was received on the old line
                rx.stop();
                if let Err(e) = rx.set_config(&config) {
                    // the handler checked it with `rate_for` and the frame match in `ww/uart.rs`
                    warn!("uart[1] config: {:?}", e);
                    cnt::cnt!(uart_config_errors: u32, warn);
                }
                let rate = line_rate();
                if Some(rate) != rate_for(config.baudrate) {
                    warn!(
                        "uart[1] runs at {} Bd, {} Bd answered",
                        rate,
                        rate_for(config.baudrate)
                    );
                    cnt::cnt!(uart_rate_mismatches: u32, warn);
                }
                defmt::info!("uart[1] at {} Bd (asked {})", rate, config.baudrate);
                APPLIED.signal(());
                continue;
            }
            Either::Second(read) => read,
        };
        match read {
            Ok(n) => {
                let fit = write_all(&RX, &buf[..n]);
                if fit < n {
                    RX_DROPPED.fetch_add((n - fit) as u32, Ordering::Relaxed);
                }
                EVENT.signal(());
            }
            Err(e) => {
                // bytes were lost in the ring (or the line broke): the next read starts the receiver again
                warn!("uart[1] rx: {:?}", e);
                cnt::cnt!(uart_rx_errors: u32, warn);
            }
        }
    }
}
