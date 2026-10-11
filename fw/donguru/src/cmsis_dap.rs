//! CMSIS-DAP v2 (bulk) USB interface for probe-rs, carrying the `dap_rs` command processor in `src/swd.rs`
//! (FW-15). Raw packets only, no reassembly across USB packets: commands and responses are capped at one USB
//! Full-Speed packet (64 bytes), matching `dap_rs`'s own `DAP2_PACKET_SIZE` without its `usb-hs` feature.
//!
//! Added to the same composite USB device as the WireWeaver interface (`src/ww.rs`) by hand, following
//! `wire_weaver_usb_embassy`'s own advice for "other USB classes alongside": build one [`embassy_usb::Builder`],
//! add every class to it, then call `.build()` once (`src/main.rs`).

use dap_rs::dap::DapVersion;
use dap_rs::usb::DAP2_PACKET_SIZE;
use embassy_futures::select::{Either, select};
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
        }
    }

    async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }

    async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.write_ep.write(data).await
    }
}

/// Runs the CMSIS-DAP v2 interface: for every bulk OUT packet from the host, process it against `host.dap` and
/// write the response back. Also services `swd::release()` requests (FW-15's Hi-Z disconnect) concurrently, so
/// a bench harness is not stuck waiting behind an idle or stalled probe-rs connection.
#[embassy_executor::task]
pub async fn dap_task(mut class: CmsisDapClass<'static, crate::ww::UsbDriver>, mut host: SwdHost) {
    let mut report = [0u8; DAP2_PACKET_SIZE as usize];
    let mut response = [0u8; DAP2_PACKET_SIZE as usize];
    loop {
        match select(class.wait_connection(), swd::wait_release_request()).await {
            Either::First(()) => {}
            Either::Second(()) => {
                host.release();
                swd::signal_release_done();
                continue;
            }
        }
        loop {
            let n = match select(class.read_packet(&mut report), swd::wait_release_request()).await {
                Either::First(Ok(n)) => n,
                Either::First(Err(EndpointError::Disabled)) => break,
                Either::First(Err(EndpointError::BufferOverflow)) => continue,
                Either::Second(()) => {
                    host.release();
                    swd::signal_release_done();
                    continue;
                }
            };
            let n_out = host.dap.process_command(&report[..n], &mut response, DapVersion::V2);
            if n_out > 0 && class.write_packet(&response[..n_out]).await.is_err() {
                break;
            }
        }
    }
}
