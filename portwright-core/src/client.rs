//! The generated `portwright_api` client: one method per item of the API tree, built by `ww_codegen!` from
//! `portwright_api/src/lib.rs`. The typed facade in [crate::levels] sits on top of it.

use wire_weaver_client::{ClientConfig, Commander, WwClient};

/// Generated client of `PortwrightApi`: the root level, `client.plug().vbus_mode().read()` and friends.
pub struct PortwrightClient {
    pub cmd: Commander,
}

impl WwClient for PortwrightClient {
    fn default_config() -> ClientConfig {
        ClientConfig::default()
    }

    fn try_from_cmd(cmd: Commander) -> Result<Self, wire_weaver_client::Error> {
        Ok(Self { cmd })
    }
}

pub mod generated {
    wire_weaver::ww_codegen!(
        portwright_api :: PortwrightApi for super::PortwrightClient,
        client = "std_client",
        // debug_to_file = "../target/portwright_client.rs",
    );
}

/// Observe configs and updates of the levels with observed properties, e.g. `PlugObserve::default().vbus(mode)`.
pub mod observe {
    pub use super::generated::portwright_api_portwright_api::portwright_api_header::{
        HeaderLatest, HeaderObserve, HeaderUpdate,
    };
    pub use super::generated::portwright_api_portwright_api::portwright_api_hub::{
        HubLatest, HubObserve, HubUpdate,
    };
    pub use super::generated::portwright_api_portwright_api::portwright_api_inputs::{
        InputsLatest, InputsObserve, InputsUpdate,
    };
    pub use super::generated::portwright_api_portwright_api::portwright_api_plug::{
        PlugLatest, PlugObserve, PlugUpdate,
    };
    pub use super::generated::portwright_api_portwright_api::portwright_api_power_meter::{
        PowerMeterLatest, PowerMeterObserve, PowerMeterUpdate,
    };
    pub use super::generated::portwright_api_portwright_api::portwright_api_sbu::{
        SbuLatest, SbuObserve, SbuUpdate,
    };
    pub use super::generated::portwright_api_portwright_api::portwright_api_swd::{
        SwdLatest, SwdObserve, SwdUpdate,
    };
    pub use wire_weaver_client::StreamEvent;
    pub use wire_weaver_client::observe::{
        Granted, ObserveEvent, ObserveMode, Observed, Observer, PreparedObserve, on_change,
        on_change_or_beat, periodic, samples,
    };
    /// A `+batched` property's slot in an update: one value, or every sample since the last update (API-39).
    pub use ww_client_server::observe::BatchedOwned;
}
