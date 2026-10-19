//! The generated `donguru_api` client: one method per item of the API tree, built by `ww_codegen!` from
//! `donguru_api/src/lib.rs`. The typed facade in [crate::levels] sits on top of it.

use wire_weaver_client::{ClientConfig, Commander, WwClient};

/// Generated client of `DonguruApi`: the root level, `client.plug().vbus_mode().read()` and friends.
pub struct DonguruClient {
    pub cmd: Commander,
}

impl WwClient for DonguruClient {
    fn default_config() -> ClientConfig {
        ClientConfig::default()
    }

    fn from_cmd(cmd: Commander) -> Self {
        Self { cmd }
    }
}

pub mod generated {
    wire_weaver::ww_codegen!(
        donguru_api :: DonguruApi for super::DonguruClient,
        client = "std_client",
        // debug_to_file = "../target/donguru_client.rs",
    );
}

/// Observe configs and updates of the levels with observed properties, e.g. `PlugObserve::default().vbus(mode)`.
pub mod observe {
    pub use super::generated::donguru_api_donguru_api::donguru_api_header::{
        HeaderLatest, HeaderObserve, HeaderUpdate,
    };
    pub use super::generated::donguru_api_donguru_api::donguru_api_hub::{
        HubLatest, HubObserve, HubUpdate,
    };
    pub use super::generated::donguru_api_donguru_api::donguru_api_inputs::{
        InputsLatest, InputsObserve, InputsUpdate,
    };
    pub use super::generated::donguru_api_donguru_api::donguru_api_plug::{
        PlugLatest, PlugObserve, PlugUpdate,
    };
    pub use super::generated::donguru_api_donguru_api::donguru_api_power_meter::{
        PowerMeterLatest, PowerMeterObserve, PowerMeterUpdate,
    };
    pub use super::generated::donguru_api_donguru_api::donguru_api_sbu::{
        SbuLatest, SbuObserve, SbuUpdate,
    };
    pub use super::generated::donguru_api_donguru_api::donguru_api_swd::{
        SwdLatest, SwdObserve, SwdUpdate,
    };
    pub use wire_weaver_client::observe::{
        ObserveEvent, ObserveMode, Observed, Observer, PreparedObserve, on_change,
        on_change_or_beat, periodic, samples,
    };
    /// A `+batched` property's slot in an update: one value, or every sample since the last update (API-39).
    pub use ww_client_server::observe::BatchedOwned;
}
