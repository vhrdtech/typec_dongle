//! The header UART, `uart[1]` (API-14): USART5 on PB3 (IO7, TX) and PB4 (IO8, RX), 8N1 at [BAUD], for
//! `src/ww/uart.rs`.
//!
//! DMA does the moving; two tasks decouple it from the server task through pipes. `tx_task` writes what the `tx`
//! sink put into [TX] and counts it in [take_tx_done], which the server task gives back to the host as credit once
//! the bytes left the pipe and went out on the line. `rx_task` reads the receiver's DMA ring into [RX]; what does not
//! fit is counted in [take_rx_dropped] and the host is told how many bytes it missed.

use core::sync::atomic::Ordering;
use defmt::{unwrap, warn};
use embassy_executor::Spawner;
use embassy_stm32::mode::Async;
use embassy_stm32::usart::{Config, Uart, UartRx, UartTx};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_sync::signal::Signal;
use portable_atomic::AtomicU32; // the M0+ has no atomic read-modify-write
use static_cell::StaticCell;

use crate::Irqs;

/// Line rate, 8N1. Fixed until `uart[].configure` is implemented.
pub const BAUD: u32 = 921_600;
/// Bytes the `tx` sink accepts ahead, its credit: the pipe [TX] holds all of them.
pub const TX_BUF: usize = 2048;
/// Received bytes held for the server task, which sends them as chunks.
pub const RX_BUF: usize = 4096;
/// The receiver's DMA ring: at [BAUD] it takes ~11 ms to fill, `rx_task` empties it far more often.
const RX_RING: usize = 1024;

static TX: Pipe<CriticalSectionRawMutex, TX_BUF> = Pipe::new();
static RX: Pipe<CriticalSectionRawMutex, RX_BUF> = Pipe::new();
/// Bytes of [TX] written out on the line since the server task looked.
static TX_DONE: AtomicU32 = AtomicU32::new(0);
/// Received bytes that did not fit in [RX] since the server task looked.
static RX_DROPPED: AtomicU32 = AtomicU32::new(0);
/// The server task has something to do: credit to give back, or received bytes to send.
static EVENT: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static RX_RING_BUF: StaticCell<[u8; RX_RING]> = StaticCell::new();

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
    loop {
        let n = TX.read(&mut buf).await;
        if let Err(e) = tx.write(&buf[..n]).await {
            warn!("uart[1] tx: {:?}", e);
        }
        TX_DONE.fetch_add(n as u32, Ordering::Relaxed);
        EVENT.signal(());
    }
}

#[embassy_executor::task]
async fn rx_task(rx: UartRx<'static, Async>, ring: &'static mut [u8; RX_RING]) {
    let mut rx = rx.into_ring_buffered(ring);
    let mut buf = [0u8; 256];
    loop {
        match rx.read(&mut buf).await {
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
