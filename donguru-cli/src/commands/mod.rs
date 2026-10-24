pub mod completions;
pub mod config;
pub mod debug;
pub mod device;
pub mod external;
pub mod fw;
pub mod gpio;
pub mod header;
pub mod info;
pub mod led;
pub mod power;
pub mod settings;
pub mod swd;
pub mod uart;
pub mod udev;
pub mod usb;

use crate::cli::Command;
use crate::context::Context;
use crate::exit::ExitCode;
use donguru_core::Selector;

pub fn dispatch(ctx: &Context, command: Command) -> anyhow::Result<ExitCode> {
    match command {
        Command::Info => info::run(ctx),
        Command::Device(cmd) => device::run(ctx, cmd),
        Command::Usb(cmd) => usb::run(ctx, cmd),
        Command::Power(cmd) => power::run(ctx, cmd),
        Command::Header(cmd) => header::run(ctx, cmd),
        Command::Gpio(cmd) => gpio::run(ctx, cmd),
        Command::Swd(cmd) => swd::run(ctx, cmd),
        Command::Led(cmd) => led::run(ctx, cmd),
        Command::Uart(cmd) => uart::run(ctx, cmd),
        Command::Settings(cmd) => settings::run(ctx, cmd),
        Command::Debug(cmd) => debug::run(ctx, cmd),
        Command::Completions(args) => completions::run(ctx, args),
        Command::Fw(cmd) => fw::run(ctx, cmd),
        Command::Udev(cmd) => udev::run(ctx, cmd),
        Command::Config(cmd) => config::run(ctx, cmd),
        Command::External(cmd) => external::run(ctx, cmd),
    }
}

pub(crate) fn stub(what: &str, exit_code: ExitCode) -> anyhow::Result<ExitCode> {
    eprintln!("donguru: `{what}` is not implemented yet");
    Ok(exit_code)
}

/// The async runtime of one command: the WireWeaver client runs its event loop on it.
pub(crate) fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?)
}

/// `--device` / `DONGURU_DEVICE`, parsed.
pub(crate) fn selector(ctx: &Context) -> anyhow::Result<Option<Selector>> {
    Ok(ctx
        .device
        .as_deref()
        .map(str::parse::<Selector>)
        .transpose()?)
}

/// Opens the selected dongle, runs `f` on it, closes the link and tears down the runtime: the shape every
/// `power` / `usb` / `header` command needs.
pub(crate) fn with_dongle<T>(
    ctx: &Context,
    f: impl AsyncFnOnce(&donguru_core::Dongle) -> donguru_core::Result<T>,
) -> anyhow::Result<T> {
    let selector = selector(ctx)?;
    let result: donguru_core::Result<T> = runtime()?.block_on(async {
        let dongle = donguru_core::Dongle::open(selector.as_ref()).await?;
        let result = f(&dongle).await;
        dongle.close().await?;
        result
    });
    Ok(result?)
}

/// `Ok(None)` for an item the firmware doesn't implement yet, instead of failing the whole command.
pub(crate) fn optional<T>(r: donguru_core::Result<T>) -> donguru_core::Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.is_unimplemented() => Ok(None),
        Err(e) => Err(e),
    }
}
