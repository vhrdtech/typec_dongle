//! `i2c[]`: 0 = plug SBU (I2C1), 1 = header IO7/IO8 (I2C3) (`ww_i2c::I2c`, API-16).

use super::ServerState;
use wire_weaver::prelude::*;

impl ServerState {
    // Both buses of the plan are there, their items answer Unimplemented until API-16.
    pub(super) fn valid_indices_root_i_2_c(&mut self) -> ValidIndices<'_> {
        ValidIndices::range_u32(0..2)
    }

    // ww_i2c::I2c at i2c[]

    pub(super) fn i2c_i2c_capabilities(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_i2c::I2cCapabilities<'_>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_configure(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _config: ww_i2c::I2cConfig,
    ) -> RpcResult<Result<ww_i2c::I2cConfig, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_write(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _addr: ww_i2c::I2cAddress,
        _data: wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 512>,
        _delay_after: Option<ww_si::Second>,
    ) -> RpcResult<Result<ww_i2c::I2cTransactionTimeRange, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_read(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _addr: ww_i2c::I2cAddress,
        _kind: ww_i2c::I2cReadKind<'_>,
        _len: u16,
        _delay_after: Option<ww_si::Second>,
    ) -> RpcResult<Result<ww_i2c::I2cReadEnvelope<'_>, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_scan(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<
        Result<
            wire_weaver::shrink_wrap::MaxLen<wire_weaver::shrink_wrap::RefVec<'_, u8>, 128>,
            ww_i2c::I2cError<'_>,
        >,
    > {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_transaction(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _addr: ww_i2c::I2cAddress,
        _ops: wire_weaver::shrink_wrap::MaxLen<
            wire_weaver::shrink_wrap::RefVec<'_, ww_i2c::I2cOp<'_>>,
            16,
        >,
    ) -> RpcResult<Result<ww_i2c::I2cTransactionResult<'_>, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_lines_state(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<ww_i2c::I2cLinesState> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_wiggle_clock(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _count: u8,
    ) -> RpcResult<Result<ww_i2c::I2cLinesState, ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_set_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
        _enabled: bool,
    ) -> RpcResult<Result<(), ww_i2c::I2cError<'_>>> {
        Unimplemented.into()
    }

    pub(super) fn i2c_i2c_pullups(
        &mut self,
        _cx: &mut Context<'_, impl EventOut>,
        _index_chain: [UNib32; 1],
    ) -> RpcResult<bool> {
        Unimplemented.into()
    }
}
