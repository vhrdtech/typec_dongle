//! CMSIS-DAP v2 request framing for the donguru firmware (`fw/donguru/src/cmsis_dap.rs`, FW-19): a DAP packet of
//! [`DAP_PACKET_SIZE`] bytes carried over full-speed bulk endpoints, so one request or response spans several USB
//! packets. No hardware and no USB stack in here: the firmware reads and writes the packets and asks this crate
//! what they mean, which is what lets the logic be unit-tested on the host.
//!
//! **Where a request ends.** A USB packet shorter than the endpoint's max packet size ends a transfer, but a host
//! does not have to send a zero-length packet after a request that is an exact multiple of it, and probe-rs 0.32
//! does not: `send_command_inner` (`probe/cmsisdap/commands/mod.rs`) hands the exact request to one nusb bulk
//! write, and nusb never sets `USBDEVFS_URB_ZERO_PACKET`. Such requests are common (a `DAP_Transfer` of 25 writes
//! is 128 bytes), so a full last packet is ambiguous on the wire. Two ways out:
//! * wait a short time for another packet, and take silence as the end: costs that time on every such request,
//!   and a host that is merely slow gets its request cut in two, the first half run against the target as if it
//!   were complete and the second half parsed as a new command;
//! * work out the request's length from its own bytes ([`shape`]), which is exact and costs nothing.
//!
//! [`Assembler`] does the second. A timeout remains for two things only: giving up on a request whose rest never
//! comes ([`Timeout::Rest`], so a lost packet or a dead host cannot leave the task waiting forever), and commands
//! [`shape`] does not know ([`Timeout::Unknown`]), where silence is the only signal left.
//!
//! **Where a response ends.** probe-rs reads into a buffer of the DAP packet size plus one, rounded up to whole
//! USB packets (`read_bulk_endpoint`, `probe/usb_util.rs`), so its read only completes on a short packet:
//! [`response_needs_zlp`].
//!
//! **Truncated requests are dropped, not run.** `dap_rs` indexes into the request without checking its length
//! (`Request::next_u8` and friends) and into the response likewise, so a request shorter than its command needs,
//! or one asking for more response than fits, would panic the firmware. [`shape`] knows both lengths for every
//! command `dap_rs` parses; the assembler only lets complete requests with a response that fits through.

#![no_std]

/// DAP packet size reported in `DAP_Info` (`MaxPacketSize`): what `dap_rs` reports with its `usb-hs` feature.
pub const DAP_PACKET_SIZE: usize = 512;

/// Response bound reported for commands whose response does not grow with the request: they all fit the 64-byte
/// DAP packet this firmware used before.
pub const SMALL_RESPONSE: usize = 64;

/// What the bytes of a request received so far say about the whole request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Total length of the request, and an upper bound for the length of its response.
    Known { request: usize, response_max: usize },
    /// The bytes that give the length have not all arrived.
    NeedMore,
    /// Not a command with a known layout (vendor commands, UART, `DAP_ExecuteCommands` / `DAP_QueueCommands`):
    /// `dap_rs` answers these without reading the request, so any length is safe to hand over.
    Unknown,
}

const fn fixed(request: usize) -> Shape {
    Shape::Known {
        request,
        response_max: SMALL_RESPONSE,
    }
}

/// Length of a request from its own bytes, per the CMSIS-DAP command layouts. `request` is what has arrived so
/// far, starting at the command byte.
pub fn shape(request: &[u8]) -> Shape {
    let Some(&command) = request.first() else {
        return Shape::NeedMore;
    };
    match command {
        0x00 => fixed(2), // DAP_Info: id
        0x01 => fixed(3), // DAP_HostStatus: type, status
        0x02 => fixed(2), // DAP_Connect: port
        0x03 => fixed(1), // DAP_Disconnect
        0x04 => fixed(6), // DAP_TransferConfigure: idle cycles, wait retry (2), match retry (2)
        0x05 => transfer(request),
        0x06 => transfer_block(request),
        0x07 => fixed(1), // DAP_TransferAbort
        0x08 => fixed(6), // DAP_WriteABORT: index, word
        0x09 => fixed(3), // DAP_Delay: microseconds (2)
        0x0A => fixed(1), // DAP_ResetTarget
        0x10 => fixed(7), // DAP_SWJ_Pins: output, select, wait (4)
        0x11 => fixed(5), // DAP_SWJ_Clock: hertz (4)
        // DAP_SWJ_Sequence: bit count (0 = 256), data
        0x12 => match request.get(1) {
            Some(&bits) => fixed(2 + count_or(bits, 256).div_ceil(8)),
            None => Shape::NeedMore,
        },
        0x13 => fixed(2), // DAP_SWD_Configure
        0x14 => sequences(request, Sequence::Jtag),
        // DAP_JTAG_Configure: device count, IR length per device
        0x15 => match request.get(1) {
            Some(&count) => fixed(2 + count as usize),
            None => Shape::NeedMore,
        },
        0x16 => fixed(2),               // DAP_JTAG_IDCODE: index
        0x17 | 0x18 | 0x1A => fixed(2), // DAP_SWO_Transport, DAP_SWO_Mode, DAP_SWO_Control
        0x19 => fixed(5),               // DAP_SWO_Baudrate: baud (4)
        0x1B => fixed(1),               // DAP_SWO_Status
        0x1C => fixed(3), // DAP_SWO_Data: max count (2); no SWO here, so no data in the response
        0x1D => sequences(request, Sequence::Swd),
        0x1E => fixed(2), // DAP_SWO_ExtendedStatus: control
        _ => Shape::Unknown,
    }
}

/// A count field where 0 stands for the field's full range.
fn count_or(count: u8, zero: usize) -> usize {
    if count == 0 { zero } else { count as usize }
}

/// DAP_Transfer: index, count, then per transfer one request byte, followed by a word for a write (also a match
/// mask write) or a read with value match. A plain read puts a word in the response instead.
fn transfer(request: &[u8]) -> Shape {
    let Some(&count) = request.get(2) else {
        return Shape::NeedMore;
    };
    let mut at = 3;
    let mut response_max = 3;
    for _ in 0..count {
        let Some(&transfer) = request.get(at) else {
            return Shape::NeedMore;
        };
        at += 1;
        let read = transfer & (1 << 1) != 0;
        let value_match = transfer & (1 << 4) != 0;
        if !read || value_match {
            at += 4;
        } else {
            response_max += 4;
        }
    }
    Shape::Known {
        request: at,
        response_max,
    }
}

/// DAP_TransferBlock: index, count (2), request byte, then one word per transfer for a write. A read puts them in
/// the response.
fn transfer_block(request: &[u8]) -> Shape {
    let (Some(&lo), Some(&hi), Some(&transfer)) = (request.get(2), request.get(3), request.get(4))
    else {
        return Shape::NeedMore;
    };
    let words = 4 * u16::from_le_bytes([lo, hi]) as usize;
    if transfer & (1 << 1) != 0 {
        Shape::Known {
            request: 5,
            response_max: 4 + words,
        }
    } else {
        Shape::Known {
            request: 5 + words,
            response_max: 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Sequence {
    Swd,
    Jtag,
}

/// DAP_SWD_Sequence and DAP_JTAG_Sequence: sequence count, then per sequence an info byte (bits 0..5: clock
/// count, 0 = 64; bit 7: capture input) and data. SWD: an output sequence carries data in the request, an input
/// sequence in the response. JTAG: TDI data always in the request, TDO in the response when captured.
fn sequences(request: &[u8], kind: Sequence) -> Shape {
    let Some(&count) = request.get(1) else {
        return Shape::NeedMore;
    };
    let mut at = 2;
    let mut response_max = 2;
    for _ in 0..count {
        let Some(&info) = request.get(at) else {
            return Shape::NeedMore;
        };
        at += 1;
        let bytes = count_or(info & 0x3F, 64).div_ceil(8);
        let capture = info & 0x80 != 0;
        if kind == Sequence::Jtag || !capture {
            at += bytes;
        }
        if capture {
            response_max += bytes;
        }
    }
    Shape::Known {
        request: at,
        response_max,
    }
}

/// Why the assembler is waiting, for the firmware to pick how long: see [`Timeout::millis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeout {
    /// The request is known to be incomplete: the rest must follow.
    Rest,
    /// A full packet of a command [`shape`] does not know: more may follow or not.
    Unknown,
}

impl Timeout {
    /// How long to wait for the next USB packet before calling [`Assembler::timeout`].
    ///
    /// The packets of one request leave the host in one bulk transfer, back to back, and the firmware re-arms
    /// the endpoint right after each one: the gap is well under a millisecond, a few at most on a busy bus.
    /// [`Timeout::Rest`] is far above that so a request is never cut short by a slow host, and far below
    /// probe-rs's 1 s USB timeout, so the stale half is gone before the host retries. [`Timeout::Unknown`] is paid
    /// on every unknown command that fills its last packet, hence shorter; cutting one of those short is harmless
    /// (see [`Shape::Unknown`]).
    pub const fn millis(self) -> u32 {
        match self {
            Timeout::Rest => 100,
            Timeout::Unknown => 10,
        }
    }
}

/// Why a request was thrown away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// The transfer ended, or its rest never came, before the request was complete.
    Truncated,
    /// The request is longer than the DAP packet: what follows is skipped until the transfer ends.
    TooLong,
    /// The response would not fit the DAP packet.
    ResponseTooLong,
}

/// What to do after a USB packet or a timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Read the next packet at [`Assembler::len`], waiting no longer than [`Assembler::pending`] says.
    Continue,
    /// The first `n` bytes of the buffer are a complete request. The assembler is empty again.
    Execute(usize),
    /// A request was thrown away, nothing to run. The assembler is ready for the next one.
    Dropped(Dropped),
}

/// Puts a DAP request together from USB packets. The buffer belongs to the caller: every packet is read to
/// `buffer[assembler.len()..]`, then reported with [`Assembler::packet`].
#[derive(Debug, Default)]
pub struct Assembler {
    len: usize,
    wait: Option<Timeout>,
    /// Inside a request that turned out too long: ignore packets until the transfer ends.
    skipping: bool,
}

impl Assembler {
    pub const fn new() -> Self {
        Assembler {
            len: 0,
            wait: None,
            skipping: false,
        }
    }

    /// Bytes of the current request received so far: where the next packet goes in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `None`: between requests, wait for as long as it takes. `Some`: inside one, call [`Assembler::timeout`]
    /// when no packet arrives in that time.
    pub fn pending(&self) -> Option<Timeout> {
        self.wait
    }

    /// Forget everything: the USB connection went away.
    pub fn reset(&mut self) {
        *self = Assembler::new();
    }

    /// A USB packet of `n` bytes was read to `buffer[self.len()..]`. `buffer` is the whole request buffer, its
    /// length the largest request accepted; `max_packet` is the endpoint's max packet size, which that length
    /// must be a multiple of so there is always room for one more packet while a request is incomplete.
    pub fn packet(&mut self, buffer: &[u8], n: usize, max_packet: usize) -> Event {
        let short = n < max_packet;
        if self.skipping {
            if short {
                self.reset();
            }
            return Event::Continue;
        }
        let len = (self.len + n).min(buffer.len());
        if len == 0 {
            // a zero-length packet between requests: a host that does terminate exact multiples
            return Event::Continue;
        }
        let full = len == buffer.len();
        match shape(&buffer[..len]) {
            Shape::Known {
                request,
                response_max,
            } if len >= request => {
                self.reset();
                if response_max > buffer.len() {
                    Event::Dropped(Dropped::ResponseTooLong)
                } else {
                    Event::Execute(len)
                }
            }
            Shape::Known { .. } | Shape::NeedMore if short => {
                self.reset();
                Event::Dropped(Dropped::Truncated)
            }
            Shape::Known { .. } | Shape::NeedMore if full => {
                self.reset();
                self.skipping = true;
                self.wait = Some(Timeout::Rest);
                Event::Dropped(Dropped::TooLong)
            }
            Shape::Unknown if short || full => {
                self.reset();
                Event::Execute(len)
            }
            Shape::Known { .. } | Shape::NeedMore => self.more(len, Timeout::Rest),
            Shape::Unknown => self.more(len, Timeout::Unknown),
        }
    }

    fn more(&mut self, len: usize, wait: Timeout) -> Event {
        self.len = len;
        self.wait = Some(wait);
        Event::Continue
    }

    /// No packet arrived within [`Assembler::pending`].
    pub fn timeout(&mut self) -> Event {
        let (len, wait, skipping) = (self.len, self.wait, self.skipping);
        self.reset();
        match wait {
            _ if skipping => Event::Continue,
            Some(Timeout::Unknown) => Event::Execute(len),
            Some(Timeout::Rest) => Event::Dropped(Dropped::Truncated),
            None => Event::Continue,
        }
    }
}

/// Whether a response of `len` bytes needs a zero-length packet after its last USB packet. A host reading more
/// than the response holds (probe-rs: see the module doc) only stops at a short packet, so a response that fills
/// its last packet needs one. Not at the full DAP packet though: a host reading exactly `dap_packet` bytes
/// (pyOCD, OpenOCD) is done with it then, and would get the zero-length packet as its next response. probe-rs
/// never asks for a response that long (it keeps `DAP_TransferBlock` to `(packet size - 6) / 4` words).
pub fn response_needs_zlp(len: usize, max_packet: usize, dap_packet: usize) -> bool {
    len > 0 && len < dap_packet && len.is_multiple_of(max_packet)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    const MPS: usize = 64;

    fn known(request: usize, response_max: usize) -> Shape {
        Shape::Known {
            request,
            response_max,
        }
    }

    /// DAP_Transfer with `writes` register writes followed by `reads` plain reads.
    fn transfer_request(writes: usize, reads: usize) -> Vec<u8> {
        let mut r = vec![0x05, 0, (writes + reads) as u8];
        for i in 0..writes {
            r.push(0x0D); // AP, write, A[3:2] = 3
            r.extend_from_slice(&(i as u32).to_le_bytes());
        }
        r.extend(core::iter::repeat_n(0x0F, reads)); // AP, read
        r
    }

    fn block_write(words: usize) -> Vec<u8> {
        let mut r = vec![0x06, 0];
        r.extend_from_slice(&(words as u16).to_le_bytes());
        r.push(0x0D);
        r.extend((0..words * 4).map(|i| i as u8));
        r
    }

    fn block_read(words: usize) -> Vec<u8> {
        let mut r = vec![0x06, 0];
        r.extend_from_slice(&(words as u16).to_le_bytes());
        r.push(0x0F);
        r
    }

    /// What the firmware task does with a host's bulk transfers, without the USB: every transfer is cut into
    /// packets of `MPS` (no zero-length packet after a full last one, like probe-rs), `None` is a silence long
    /// enough for the timeout. Returns the executed requests and the drops, in order.
    fn run(transfers: &[Option<&[u8]>]) -> Vec<Result<Vec<u8>, Dropped>> {
        let mut buffer = [0u8; DAP_PACKET_SIZE];
        let mut asm = Assembler::new();
        let mut out = Vec::new();
        let mut record = |event: Event, buffer: &[u8]| match event {
            Event::Continue => {}
            Event::Execute(n) => out.push(Ok(buffer[..n].to_vec())),
            Event::Dropped(why) => out.push(Err(why)),
        };
        for transfer in transfers {
            match transfer {
                Some(data) => {
                    let mut chunks: Vec<&[u8]> = data.chunks(MPS).collect();
                    if data.is_empty() {
                        chunks.push(&[]);
                    }
                    for chunk in chunks {
                        let at = asm.len();
                        assert!(buffer.len() - at >= MPS, "no room for a packet at {at}");
                        buffer[at..at + chunk.len()].copy_from_slice(chunk);
                        record(asm.packet(&buffer, chunk.len(), MPS), &buffer);
                    }
                }
                None => {
                    if asm.pending().is_some() {
                        record(asm.timeout(), &buffer);
                    }
                }
            }
        }
        out
    }

    #[test]
    fn fixed_commands() {
        assert_eq!(shape(&[0x00, 0xFF]), fixed(2));
        assert_eq!(shape(&[0x00]), fixed(2));
        assert_eq!(shape(&[0x03]), fixed(1));
        assert_eq!(shape(&[0x11, 0x40, 0x42, 0x0F, 0x00]), fixed(5));
        assert_eq!(shape(&[]), Shape::NeedMore);
    }

    #[test]
    fn swj_sequence() {
        assert_eq!(shape(&[0x12]), Shape::NeedMore);
        assert_eq!(shape(&[0x12, 51]), fixed(2 + 7));
        assert_eq!(shape(&[0x12, 8]), fixed(3));
        assert_eq!(shape(&[0x12, 0]), fixed(2 + 32));
    }

    #[test]
    fn swd_sequence() {
        // 52 bits out (7 bytes), 8 bits in, 64 bits out (count 0)
        let r = [0x1D, 3, 52, 1, 2, 3, 4, 5, 6, 7, 0x88, 0x00];
        assert_eq!(shape(&r), known(2 + 1 + 7 + 1 + 1 + 8, 2 + 1));
        assert_eq!(shape(&r[..5]), Shape::NeedMore);
        assert_eq!(shape(&[0x1D]), Shape::NeedMore);
        assert_eq!(shape(&[0x1D, 0]), known(2, 2));
    }

    #[test]
    fn jtag_sequence_always_carries_tdi() {
        // 8 bits with TDO capture, 9 bits without
        assert_eq!(
            shape(&[0x14, 2, 0x88, 0xAA, 0x09]),
            known(2 + 2 + 1 + 2, 2 + 1)
        );
    }

    #[test]
    fn transfer() {
        assert_eq!(shape(&[0x05, 0]), Shape::NeedMore);
        assert_eq!(shape(&[0x05, 0, 0]), known(3, 3));
        assert_eq!(shape(&transfer_request(25, 0)), known(128, 3));
        assert_eq!(shape(&transfer_request(3, 2)), known(3 + 15 + 2, 3 + 8));
        // the length is known as soon as every request byte is in, here before the last write's data
        assert_eq!(shape(&transfer_request(25, 0)[..124]), known(128, 3));
        assert_eq!(shape(&transfer_request(25, 0)[..123]), Shape::NeedMore);
        // read with value match carries the value and returns no data; a match mask write carries the mask
        let r = [0x05, 0, 2, 0x20, 1, 2, 3, 4, 0x12, 5, 6, 7, 8];
        assert_eq!(shape(&r), known(13, 3));
    }

    #[test]
    fn transfer_block() {
        assert_eq!(shape(&[0x06, 0, 126, 0]), Shape::NeedMore);
        assert_eq!(shape(&block_read(126)), known(5, 508));
        assert_eq!(shape(&block_write(126)[..64]), known(509, 4));
        assert_eq!(shape(&block_read(0xFFFF)), known(5, 4 + 4 * 0xFFFF));
    }

    #[test]
    fn unknown_commands() {
        for command in [0x1F, 0x21, 0x7E, 0x7F, 0x80, 0xFF] {
            assert_eq!(shape(&[command, 1, 2, 3]), Shape::Unknown);
        }
    }

    #[test]
    fn short_packet_is_one_request() {
        let info = [0x00, 0xFF];
        assert_eq!(run(&[Some(&info)]), [Ok(info.to_vec())]);
    }

    #[test]
    fn exact_multiple_needs_no_timeout() {
        let request = transfer_request(25, 0);
        assert_eq!(request.len(), 2 * MPS);
        let next = [0x00, 0xFE];
        // no `None` (silence) between the two: the second must not be glued to the first
        assert_eq!(
            run(&[Some(&request), Some(&next)]),
            [Ok(request.clone()), Ok(next.to_vec())]
        );
    }

    #[test]
    fn exact_single_packet() {
        // 12 writes and one read: 3 + 60 + 1
        let request = transfer_request(12, 1);
        assert_eq!(request.len(), MPS);
        assert_eq!(run(&[Some(&request)]), [Ok(request.clone())]);
    }

    #[test]
    fn largest_probe_rs_requests() {
        let write = block_write(126);
        assert_eq!(write.len(), 509);
        let batch = transfer_request(101, 0);
        assert_eq!(batch.len(), 508);
        assert_eq!(
            run(&[Some(&write), Some(&batch), Some(&block_read(126))]),
            [Ok(write.clone()), Ok(batch.clone()), Ok(block_read(126))]
        );
    }

    #[test]
    fn full_dap_packet() {
        // 101 writes and 4 reads: 3 + 505 + 4
        let request = transfer_request(101, 4);
        assert_eq!(request.len(), DAP_PACKET_SIZE);
        assert_eq!(run(&[Some(&request)]), [Ok(request.clone())]);
    }

    #[test]
    fn zero_length_packets_are_ignored() {
        let request = transfer_request(25, 0);
        let info = [0x00, 0xFF];
        assert_eq!(
            run(&[Some(&[]), Some(&request), Some(&[]), Some(&info)]),
            [Ok(request.clone()), Ok(info.to_vec())]
        );
    }

    #[test]
    fn truncated_by_short_packet_is_dropped() {
        let write = block_write(126);
        let info = [0x00, 0xFF];
        assert_eq!(
            run(&[Some(&write[..100]), Some(&info)]),
            [Err(Dropped::Truncated), Ok(info.to_vec())]
        );
        // dap_rs would index past the end of these
        assert_eq!(run(&[Some(&[0x00])]), [Err(Dropped::Truncated)]);
        assert_eq!(run(&[Some(&[0x05, 0, 1])]), [Err(Dropped::Truncated)]);
        assert_eq!(
            run(&[Some(&[0x06, 0, 1, 0, 0x0D, 1, 2])]),
            [Err(Dropped::Truncated)]
        );
    }

    #[test]
    fn lost_rest_resyncs_on_timeout() {
        let write = block_write(126);
        let info = [0x00, 0xFF];
        assert_eq!(
            run(&[Some(&write[..128]), None, Some(&info)]),
            [Err(Dropped::Truncated), Ok(info.to_vec())]
        );
    }

    #[test]
    fn without_the_timeout_the_next_request_would_be_swallowed() {
        // why the firmware must honour `pending()`: the stale half takes the next request as its data
        let write = block_write(126);
        assert_eq!(
            run(&[Some(&write[..128]), Some(&[0x00, 0xFF])]),
            [Err(Dropped::Truncated)]
        );
    }

    #[test]
    fn too_long_is_skipped_to_the_end_of_the_transfer() {
        let write = block_write(200); // 805 bytes
        let info = [0x00, 0xFF];
        assert_eq!(
            run(&[Some(&write), Some(&info)]),
            [Err(Dropped::TooLong), Ok(info.to_vec())]
        );
        // its last packet full: only silence ends the skipping
        let write = block_write(207);
        let write = &write[..13 * MPS];
        assert_eq!(
            run(&[Some(write), None, Some(&info)]),
            [Err(Dropped::TooLong), Ok(info.to_vec())]
        );
    }

    #[test]
    fn response_too_long_is_dropped() {
        assert_eq!(run(&[Some(&block_read(127))]), [Ok(block_read(127))]);
        assert_eq!(
            run(&[Some(&block_read(128))]),
            [Err(Dropped::ResponseTooLong)]
        );
        let reads = transfer_request(0, 128);
        assert_eq!(run(&[Some(&reads)]), [Err(Dropped::ResponseTooLong)]);
        let reads = transfer_request(0, 127);
        assert_eq!(run(&[Some(&reads)]), [Ok(reads.clone())]);
    }

    #[test]
    fn unknown_command_ends_on_silence_or_short_packet() {
        let mut vendor = vec![0x80u8; 2 * MPS];
        let info = [0x00, 0xFF];
        assert_eq!(
            run(&[Some(&vendor), None, Some(&info)]),
            [Ok(vendor.clone()), Ok(info.to_vec())]
        );
        vendor.push(1);
        assert_eq!(
            run(&[Some(&vendor), Some(&info)]),
            [Ok(vendor.clone()), Ok(info.to_vec())]
        );
        let full = vec![0x80u8; DAP_PACKET_SIZE];
        assert_eq!(
            run(&[Some(&full), Some(&info)]),
            [Ok(full.clone()), Ok(info.to_vec())]
        );
    }

    #[test]
    fn pending_says_how_long_to_wait() {
        let mut buffer = [0u8; DAP_PACKET_SIZE];
        let mut asm = Assembler::new();
        assert_eq!(asm.pending(), None);
        buffer[..MPS].copy_from_slice(&block_write(126)[..MPS]);
        assert_eq!(asm.packet(&buffer, MPS, MPS), Event::Continue);
        assert_eq!((asm.len(), asm.pending()), (MPS, Some(Timeout::Rest)));
        asm.reset();
        assert!(asm.is_empty());
        buffer[..MPS].fill(0x80);
        assert_eq!(asm.packet(&buffer, MPS, MPS), Event::Continue);
        assert_eq!(asm.pending(), Some(Timeout::Unknown));
        assert_eq!(asm.timeout(), Event::Execute(MPS));
        assert_eq!(asm.pending(), None);
        assert_eq!(asm.timeout(), Event::Continue);
    }

    #[test]
    fn padded_request_still_runs() {
        // a host padding to whole packets, as CMSIS-DAP v1 hosts do
        let mut padded = vec![0u8; MPS];
        padded[..2].copy_from_slice(&[0x00, 0xFF]);
        assert_eq!(run(&[Some(&padded)]), [Ok(padded.clone())]);
    }

    #[test]
    fn garbage_never_panics_or_overruns() {
        // xorshift: any bytes in any packet sizes; `run` asserts there is always room for the next packet
        let mut x = 0x2545_F491u32;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        for _ in 0..2000 {
            let len = next() as usize % 1100;
            let mut data: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            if let Some(first) = data.first_mut() {
                *first %= 0x20; // mostly real command ids
            }
            // sometimes no silence after it: whatever state is left must not break the next request either
            let silence = (next() % 4 != 0).then_some(None);
            let transfers: Vec<Option<&[u8]>> =
                core::iter::once(Some(&data[..])).chain(silence).collect();
            for result in run(&transfers).into_iter().flatten() {
                assert!(result.len() <= DAP_PACKET_SIZE);
                if let Shape::Known {
                    request,
                    response_max,
                } = shape(&result)
                {
                    assert!(request <= result.len() && response_max <= DAP_PACKET_SIZE);
                }
            }
        }
    }

    #[test]
    fn response_zlp() {
        assert!(!response_needs_zlp(4, MPS, DAP_PACKET_SIZE));
        assert!(!response_needs_zlp(508, MPS, DAP_PACKET_SIZE));
        assert!(response_needs_zlp(64, MPS, DAP_PACKET_SIZE));
        assert!(response_needs_zlp(448, MPS, DAP_PACKET_SIZE));
        assert!(!response_needs_zlp(512, MPS, DAP_PACKET_SIZE));
        assert!(!response_needs_zlp(0, MPS, DAP_PACKET_SIZE));
    }
}
