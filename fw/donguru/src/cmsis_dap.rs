//! CMSIS-DAP v2 (bulk) USB interface for probe-rs, carrying the `dap_rs` command processor in `src/swd.rs`
//! (FW-15). The DAP packet is 512 bytes (`dap_rs`'s `usb-hs` feature, which only changes the size it reports in
//! `DAP_Info`) over Full-Speed 64-byte bulk endpoints (FW-19): a request is put together from several OUT packets
//! and a response split over several IN packets. Where a request ends, what happens to a truncated one and when
//! a response gets a zero-length packet is decided in the `donguru_dap` crate (repo root), which has the reasoning
//! and the unit tests; this file only moves the packets.
//!
//! Added to the same composite USB device as the WireWeaver interface (`src/ww.rs`) by hand, following
//! `wire_weaver_usb_embassy`'s own advice for "other USB classes alongside": build one [`embassy_usb::Builder`],
//! add every class to it, then call `.build()` once (`src/main.rs`).

use cnt::cnt;
use dap_rs::dap::{Command, DapVersion};
use donguru_dap::{Assembler, DAP_PACKET_SIZE, Dropped, Event, response_needs_zlp};
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_time::{Duration, Timer, with_timeout};
use embassy_usb::Builder;
use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::types::StringIndex;
use embassy_usb::{Handler, types::InterfaceNumber};

use crate::swd::{self, SwdHost};

const USB_CLASS_VENDOR_SPECIFIC: u8 = 0xFF;
const USB_SUBCLASS_NONE: u8 = 0x00;
const USB_PROTOCOL_NONE: u8 = 0x00;
/// Not part of the CMSIS-DAP spec as a class/subclass/protocol triplet: probes identify the interface by this
/// substring in its string descriptor (what dap-rs's own `usb::dap_v2` module uses, and what probe-rs scans for).
const INTERFACE_STRING: &str = "CMSIS-DAP v2 Interface";

// `dap_rs` reports its own constant in `DAP_Info`, the buffers here are sized by ours.
const _: () = assert!(dap_rs::usb::DAP2_PACKET_SIZE as usize == DAP_PACKET_SIZE);

/// How long the host may take to read a whole response. probe-rs gives up on its read after 1 s (50 ms while
/// probing the packet size), so nobody is waiting for the rest after that; without a limit a host that stopped
/// reading would keep this task, and with it `swd::release()`, stuck in the write.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);

/// Serves [`INTERFACE_STRING`], must outlive the [`CmsisDapClass`] built with it.
pub struct State {
    name_index: Option<StringIndex>,
}

impl State {
    pub const fn new() -> Self {
        State { name_index: None }
    }
}

impl Handler for State {
    fn get_string(&mut self, index: StringIndex, _lang_id: u16) -> Option<&str> {
        (Some(index) == self.name_index).then_some(INTERFACE_STRING)
    }
}

pub struct CmsisDapClass<'d, D: Driver<'d>> {
    _interface: InterfaceNumber,
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
    max_packet_size: usize,
}

impl<'d, D: Driver<'d>> CmsisDapClass<'d, D> {
    /// Adds the CMSIS-DAP v2 function and interface to `builder`: one OUT and one IN bulk endpoint, no HID (v1)
    /// fallback and no SWO trace endpoint (not wired on this hardware, see `src/swd.rs`).
    pub fn new(builder: &mut Builder<'d, D>, state: &'d mut State, max_packet_size: u16) -> Self {
        let mut func = builder.function(USB_CLASS_VENDOR_SPECIFIC, USB_SUBCLASS_NONE, USB_PROTOCOL_NONE);
        let mut iface = func.interface();
        let interface = iface.interface_number();
        let name_index = iface.string();
        state.name_index = Some(name_index);
        let mut alt = iface.alt_setting(
            USB_CLASS_VENDOR_SPECIFIC,
            USB_SUBCLASS_NONE,
            USB_PROTOCOL_NONE,
            Some(name_index),
        );
        let read_ep = alt.endpoint_bulk_out(None, max_packet_size);
        let write_ep = alt.endpoint_bulk_in(None, max_packet_size);
        drop(func);
        builder.handler(state);

        CmsisDapClass {
            _interface: interface,
            read_ep,
            write_ep,
            max_packet_size: max_packet_size as usize,
        }
    }

    async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }

    async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    /// One response as one bulk transfer: whole packets, then a short or zero-length one.
    async fn write_response(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        for packet in data.chunks(self.max_packet_size) {
            self.write_ep.write(packet).await?;
        }
        if response_needs_zlp(data.len(), self.max_packet_size, DAP_PACKET_SIZE) {
            self.write_ep.write(&[]).await?;
        }
        Ok(())
    }
}

/// Waits for the assembler's timeout, or forever between requests.
async fn expire(after: Option<donguru_dap::Timeout>) {
    match after {
        Some(timeout) => Timer::after(Duration::from_millis(timeout.millis() as u64)).await,
        None => core::future::pending().await,
    }
}

/// Runs the CMSIS-DAP v2 interface: puts every request from the host together from its bulk OUT packets,
/// processes it against `host.dap` and writes the response back. Also services `swd::release()` requests
/// (FW-15's Hi-Z disconnect) concurrently, so a bench harness is not stuck waiting behind an idle or stalled
/// probe-rs connection.
///
/// A request that never completes is dropped when the assembler's timeout runs out, so the next one starts
/// clean; dropped requests get no response (the host runs into its own timeout) and are counted.
///
/// A clean `DAP_Disconnect` already releases the SWD-host pins to Hi-Z, inside `host.dap.process_command`
/// (`dap_rs` calls `SwdPins::high_impedance_mode` on its own state machine). But a session can also end without
/// one: the host process dying, a cable pull, or any other USB reset/suspend that disables this interface's
/// endpoints. [`EndpointError::Disabled`] from either endpoint covers that case, so a bench harness only has to
/// detach probe-rs (ending the session one way or another) before cutting the DUT's power, without also having
/// to remember an explicit `swd_release` call; `swd_release` stays for cases that want it mid-session.
#[embassy_executor::task]
pub async fn dap_task(mut class: CmsisDapClass<'static, crate::ww::UsbDriver>, mut host: SwdHost) {
    let mut request = [0u8; DAP_PACKET_SIZE];
    let mut response = [0u8; DAP_PACKET_SIZE];
    let mut assembler = Assembler::new();
    let max_packet_size = class.max_packet_size;
    loop {
        match select(class.wait_connection(), swd::wait_release_request()).await {
            Either::First(()) => {}
            Either::Second(()) => {
                host.release();
                swd::signal_release_done();
                continue;
            }
        }
        assembler.reset();
        loop {
            // Always room for a packet: an incomplete request is a whole number of packets short of the buffer.
            let packet = class.read_packet(&mut request[assembler.len()..]);
            let event = match select3(packet, swd::wait_release_request(), expire(assembler.pending())).await {
                Either3::First(Ok(n)) => assembler.packet(&request, n, max_packet_size),
                Either3::First(Err(EndpointError::Disabled)) => {
                    host.release();
                    break;
                }
                Either3::First(Err(EndpointError::BufferOverflow)) => {
                    cnt!(dap_usb_overflows: u32);
                    assembler.reset();
                    continue;
                }
                Either3::Second(()) => {
                    host.release();
                    swd::signal_release_done();
                    continue;
                }
                Either3::Third(()) => assembler.timeout(),
            };
            let n = match event {
                Event::Continue => continue,
                Event::Execute(n) => n,
                Event::Dropped(Dropped::Truncated) => {
                    cnt!(dap_requests_truncated: u32);
                    continue;
                }
                Event::Dropped(Dropped::TooLong) => {
                    cnt!(dap_requests_too_long: u32);
                    continue;
                }
                Event::Dropped(Dropped::ResponseTooLong) => {
                    cnt!(dap_responses_too_long: u32);
                    continue;
                }
            };
            let n_out = host.dap.process_command(&request[..n], &mut response, DapVersion::V2);
            // Stopgap plug CC drive (FW-17, P2534#12oct-1338b, `src/swd.rs` module doc): hardcoded to these two
            // command bytes rather than a real connect/disconnect callback through `dap_rs`. `response[1]` is
            // `ConnectPortResponse`, 0 = Failed.
            match Command::try_from(request[0]) {
                Ok(Command::DAP_Connect) if n_out >= 2 && response[1] != 0 => host.drive_cc(),
                Ok(Command::DAP_Disconnect) => host.release(),
                _ => {}
            }
            if n_out == 0 {
                continue;
            }
            match with_timeout(RESPONSE_TIMEOUT, class.write_response(&response[..n_out])).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {
                    host.release();
                    break;
                }
                Err(_) => {
                    cnt!(dap_response_timeouts: u32);
                }
            }
        }
    }
}
