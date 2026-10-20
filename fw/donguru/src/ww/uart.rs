//! `uart[]`: 0 = plug SBU, 1 = header IO7/IO8 (`ww_uart::Uart`, API-14).

use super::ServerState;
use super::UsbDriver;
use crate::uart;
use embassy_time::Instant;
use static_cell::StaticCell;
use wire_weaver::prelude::*;
use wire_weaver::shrink_wrap::Error as SwError;
use wire_weaver_usb_embassy::UsbServer;
use ww_client_server::stream::{StreamCodec, StreamCodecs, StreamStatus};
use ww_stream::codec::{Compressed, CountedState};
use ww_stream::tx::Counted;
use ww_stream::{Opened, ResumeError, StreamCursor, StreamHead};
use ww_uart::{RxChunk, UART_CHUNK_MAX};

/// `uart[]` index of the header UART, the only one with hardware behind it so far.
const HEADER: usize = 1;

/// The header IO level, fixed: the MCU's 3.3 V.
const IO_LEVEL: ww_si::Volt = ww_si::Volt::from_parts(
    ww_si::Prefix::Milli,
    ww_si::ww_numeric::NumericValue::U32(3300),
);

/// Indices of `uart[]`: 0 = plug SBU, 1 = header.
pub(super) const COUNT: usize = 2;

/// `uart[1].rx` with its LZ4 encoder (WireWeaver API-55, FW-42): 4096 hash entries (8 KiB) and 12 KiB of history and
/// input, 4 KiB of it kept when the buffer slides. Compresses for hosts that offer LZ4, sends as is to the others.
type HeaderRx = Compressed<ww_lz4::Encoder<4096, 12288>>;
/// Set up in place: as a const it would keep its 20 KiB as an initializer in flash.
static HEADER_RX: StaticCell<HeaderRx> = StaticCell::new();

/// State of a `uart[].rx`: the header's compressed stream, or a plain one never opened (the plug UART, API-14).
pub(super) enum RxState {
    Unopened(Counted),
    Header(&'static mut HeaderRx),
}

impl RxState {
    /// The states of `uart[]`, by index; once only.
    pub(super) fn new() -> [RxState; COUNT] {
        [
            RxState::Unopened(Counted::new()),
            RxState::Header(Compressed::init(HEADER_RX.uninit(), |c| {
                ww_lz4::Encoder::init(c, 4096)
            })),
        ]
    }
}

impl CountedState for RxState {
    fn counted(&mut self) -> &mut Counted {
        match self {
            RxState::Unopened(s) => s,
            RxState::Header(s) => s.counted(),
        }
    }

    fn open_offered(
        &mut self,
        session: u32,
        codecs: Option<&StreamCodecs>,
    ) -> (Opened, Option<StreamCodec>) {
        match self {
            RxState::Unopened(s) => s.open_offered(session, codecs),
            RxState::Header(s) => s.open_offered(session, codecs),
        }
    }

    fn resume_offered(
        &mut self,
        cursor: StreamCursor,
        codecs: Option<&StreamCodecs>,
    ) -> Result<(Opened, Option<StreamCodec>), ResumeError<()>> {
        match self {
            RxState::Unopened(s) => s.resume_offered(cursor, codecs),
            RxState::Header(s) => s.resume_offered(cursor, codecs),
        }
    }

    fn detach(&mut self) {
        match self {
            RxState::Unopened(s) => CountedState::detach(s),
            RxState::Header(s) => s.detach(),
        }
    }

    fn status(&mut self) -> StreamStatus {
        match self {
            RxState::Unopened(s) => CountedState::status(s),
            RxState::Header(s) => s.status(),
        }
    }

    fn head(&mut self, units: u64, t: Option<ww_si::Second>) -> Option<StreamHead> {
        match self {
            RxState::Unopened(s) => CountedState::head(s, units, t),
            RxState::Header(s) => s.head(units, t),
        }
    }

    fn write_item(
        &mut self,
        buf: &mut [u8],
        item: impl FnOnce(&mut [u8]) -> Result<usize, SwError>,
    ) -> Result<usize, SwError> {
        match self {
            RxState::Unopened(s) => s.write_item(buf, item),
            RxState::Header(s) => s.write_item(buf, item),
        }
    }

    fn unsent(&mut self, head: &StreamHead) {
        match self {
            RxState::Unopened(s) => CountedState::unsent(s, head),
            RxState::Header(s) => s.unsent(head),
        }
    }
}

impl ServerState {
    // Both indices of the plan are there, their items answer Unimplemented until API-14.
    pub(super) fn valid_indices_root_uart(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..COUNT as u32)
    }

    // ww_uart::Uart at uart[]

    pub(super) fn uart_rx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        index_chain: [UNib32; 1],
    ) -> RpcResult<()> {
        if index_chain[0].0 as usize == HEADER {
            Ready(())
        } else {
            RpcResult::Unimplemented
        }
    }

    pub(super) fn close_uart_rx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) {
    }

    pub(super) fn stream_uart_rx(&mut self, index_chain: [UNib32; 1]) -> &mut RxState {
        // the generated code checked the index against `valid_indices_root_uart`
        &mut self.uart_rx[(index_chain[0].0 as usize).min(COUNT - 1)]
    }

    pub(super) fn credit_uart_tx(
        &mut self,
        index_chain: [UNib32; 1],
    ) -> &mut ww_stream::sink::SinkCredit {
        // the generated code checked the index against `valid_indices_root_uart`
        let i = (index_chain[0].0 as usize).min(COUNT - 1);
        &mut self.uart_tx_credit[i]
    }

    pub(super) fn sideband_uart_tx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _sideband: ww_client_server::StreamSideband,
    ) -> Option<ww_client_server::StreamSideband> {
        // TODO: None sends no reply
        None
    }

    pub(super) fn write_uart_tx(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        index_chain: [UNib32; 1],
        data: &wire_weaver::shrink_wrap::tail_bytes::TailBytes<'_>,
    ) {
        // the other index has no credit, so nothing arrives for it
        if index_chain[0].0 as usize == HEADER && uart::tx_write(data.0) < data.0.len() {
            // the host kept to its credit, the pipe holds all of it
            cnt::cnt!(uart_tx_overruns: u32, warn);
        }
    }

    pub(super) fn uart_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        index_chain: [UNib32; 1],
    ) -> RpcResult<ww_uart::Capabilities<'_>> {
        if index_chain[0].0 as usize != HEADER {
            return Unimplemented.into();
        }
        Ready(ww_uart::Capabilities {
            min_baud_rate: ww_uart::Baud::base(uart::MIN_BAUD),
            max_baud_rate: ww_uart::Baud::base(uart::MAX_BAUD),
            io_levels: MaxLen::new_unchecked(RefVec::Slice {
                slice: &const { [IO_LEVEL] },
            }),
            io_level_min: None,
            io_level_max: None,
            hw_flow_control: false,
            sw_flow_control: false,
            high_z_mode: false,
            test_mode: false,
            back_feed_detector: false,
            rx_buffer: uart::RX_BUF as u32,
            tx_buffer: uart::TX_BUF as u32,
        })
    }

    pub(super) fn uart_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        index_chain: [UNib32; 1],
        config: ww_uart::UartConfig,
    ) -> RpcResult<Result<ww_uart::UartConfig, ww_uart::UartError<'_>>> {
        if index_chain[0].0 as usize != HEADER {
            return Unimplemented.into();
        }
        let unsupported = Ready(Err(ww_uart::UartError::Common(CommonError::Unsupported)));
        let Some((line, rate)) = line_config(&config) else {
            return unsupported;
        };
        uart::configure(line);
        Ready(Ok(ww_uart::UartConfig {
            baud_rate: ww_uart::Baud::base(rate),
            io_level: Some(IO_LEVEL),
            // the receiver's IDLE flag: one character time
            idle_chars: 1,
            ..config
        }))
    }

    pub(super) fn uart_set_pin_level(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _pin: ww_uart::Pin,
        _is_high: bool,
    ) -> RpcResult<Result<(), ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn uart_send_break(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _duration: ww_si::Second,
    ) -> RpcResult<Result<(), ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn uart_tx_pending(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<u32> {
        Unimplemented.into()
    }

    // optional, can be left out with omit = ["uart.monitor"]
    pub(super) fn uart_monitor(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_uart::MonitorConfig,
    ) -> RpcResult<Result<ww_uart::MonitorConfig, ww_uart::UartError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn close_uart_monitor(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) {
    }

    pub(super) fn stream_uart_monitor(
        &mut self,
        _index_chain: [UNib32; 1],
    ) -> &mut ww_stream::tx::Counted {
        &mut self.unopened
    }
}

impl ServerState {
    /// The header UART has something for the host: credit to give back for bytes sent on the line, bytes received.
    /// Called by the server task when [uart::wait_event] completes.
    pub(super) async fn uart_pump(&mut self, server: &mut UsbServer<'static, UsbDriver>) {
        let done = uart::take_tx_done();
        if done > 0 {
            self.uart_tx_credit[HEADER].free(done);
            if server.is_up()
                && let Some(ack) = self.uart_tx_credit[HEADER].take_ack()
            {
                let mut out = server.sink();
                let r = super::server_impl::stream_data_ser()
                    .uart(HEADER as u32)
                    .tx_sideband_send(ack, &mut out)
                    .await;
                if r.is_err() {
                    cnt::cnt!(uart_ack_errors: u32);
                }
            }
        }
        let dropped = uart::take_rx_dropped();
        if dropped > 0 {
            self.uart_rx[HEADER].counted().lost(dropped as u64);
        }
        let mut buf = [0u8; UART_CHUNK_MAX];
        loop {
            let n = uart::read_rx(&mut buf);
            if n == 0 {
                break;
            }
            let bytes = &buf[..n];
            // no host (or the link is down): rx_send counts the bytes as lost, the host sees how many it missed
            let chunk = RxChunk {
                // the DMA ring is read in turns, not when the last byte arrived: no time of its own
                end: None,
                flags: None,
                bytes: MaxLen::new_unchecked(RefVec::new_bytes(bytes)),
            };
            let first = ww_si::Second::micro(Instant::now().as_micros() as u32);
            let mut out = server.sink();
            let r = super::server_impl::stream_data_ser()
                .uart(HEADER as u32)
                .rx_send(
                    &mut self.uart_rx[HEADER],
                    &chunk,
                    n as u64,
                    Some(first),
                    &mut out,
                )
                .await;
            if r.is_err() {
                cnt::cnt!(uart_rx_send_errors: u32);
            }
        }
    }
}

/// The USART5 setting for `config` and the rate it runs at, `None` for what the header UART can't do: a rate outside
/// `MIN_BAUD..=MAX_BAUD`, 9 data bits (the pipes carry bytes) or fewer than 7, flow control, high-Z, test mode, the
/// back-feed detector, an IO level other than 3.3 V.
fn line_config(config: &ww_uart::UartConfig) -> Option<(uart::LineConfig, u32)> {
    use embassy_stm32::usart;
    use ww_uart::{DataBits, Mode, Parity, StopBits};
    let baud = config.baud_rate.to_u32_in(ww_si::Prefix::Unit)?;
    let rate = uart::rate_for(baud)?;
    if config.mode != Mode::Asynchronous || config.prevent_back_feed {
        return None;
    }
    if let Some(level) = &config.io_level
        && level.to_u32_in(ww_si::Prefix::Milli) != Some(3300)
    {
        return None;
    }
    let mut line = usart::Config::default();
    line.baudrate = baud;
    line.data_bits = match config.data_bits {
        DataBits::Eight => usart::DataBits::DataBits8,
        DataBits::Seven => usart::DataBits::DataBits7,
        _ => return None,
    };
    line.parity = match config.parity {
        Parity::None => usart::Parity::ParityNone,
        Parity::Even => usart::Parity::ParityEven,
        Parity::Odd => usart::Parity::ParityOdd,
        _ => return None,
    };
    line.stop_bits = match config.stop_bits {
        StopBits::Stop1 => usart::StopBits::STOP1,
        StopBits::Stop2 => usart::StopBits::STOP2,
    };
    Some((line, rate))
}
