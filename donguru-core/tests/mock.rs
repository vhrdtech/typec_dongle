//! donguru-core against the in-process mock (CORE-3): every level through the typed facade, device errors with
//! their category and typed value, `Unimplemented`, link failures and observe.

use std::time::Duration;

use donguru_core::api::*;
use donguru_core::client::observe::{PlugObserve, PowerMeterObserve, on_change};
use donguru_core::levels::Side;
use donguru_core::mock::MockDevice;
use donguru_core::{DeviceErrorKind, Dongle, Error};
use ww_si::Prefix;

fn mv(v: i32) -> Volt {
    Volt::new(Prefix::Milli, v)
}

fn ms(v: i32) -> Second {
    Second::new(Prefix::Milli, v)
}

async fn start(path: &str) -> (MockDevice, Dongle) {
    // a device answering with an error outside an item's #[errors(..)] fails the test
    wire_weaver_client::set_strict_error_lists(true);
    let mock = MockDevice::start(path);
    let dongle = mock.open().await.expect("open the mock");
    (mock, dongle)
}

#[track_caller]
fn device_kind(e: Error) -> DeviceErrorKind {
    e.device_kind()
        .unwrap_or_else(|| panic!("expected a device error, got {e:?}"))
}

#[track_caller]
fn unimplemented<T: std::fmt::Debug>(r: donguru_core::Result<T>) {
    match r {
        Err(e) if e.is_unimplemented() => {}
        other => panic!("expected Unimplemented, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn device_contract() {
    let (mock, dongle) = start("core/device").await;
    let device = dongle.device();
    assert_eq!(device.variant().await.unwrap(), BoardVariant::Main);
    let board = device.board().await.unwrap();
    assert_eq!(
        (board.name.as_str(), board.revision.as_str()),
        ("Donguru B153", "A")
    );
    assert_eq!(device.uid().await.unwrap(), "0049002A3133510B37363436");
    assert_eq!(
        device.firmware().await.unwrap().name.as_str(),
        "donguru-mock"
    );

    device.set_label("bench 2").await.unwrap();
    assert_eq!(device.label().await.unwrap(), "bench 2");
    assert_eq!(mock.state().await.label, "bench 2");
    assert!(
        device.set_label(&"x".repeat(33)).await.is_err(),
        "label longer than 32 bytes"
    );

    // what the firmware doesn't have yet is None in the summary, not an error
    let summary = device.summary().await.unwrap();
    assert_eq!(summary.variant, BoardVariant::Main);
    assert_eq!(summary.label.as_deref(), Some("bench 2"));
    assert!(summary.last_reset.is_none());
    unimplemented(device.last_reset().await);

    mock.with(|s| s.variant = BoardVariant::Plus).await;
    assert_eq!(device.variant().await.unwrap(), BoardVariant::Plus);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings() {
    let (mock, dongle) = start("core/settings").await;
    let settings = dongle.device().settings();
    assert_eq!(
        settings.plug_vbus_at_boot().await.unwrap(),
        VbusMode::ForceOff
    );
    let on = VbusMode::ForceOn {
        max_voltage: mv(5_500),
    };
    settings.set_plug_vbus_at_boot(on.clone()).await.unwrap();
    assert_eq!(settings.plug_vbus_at_boot().await.unwrap(), on);
    // the boot setting doesn't switch anything now
    assert_eq!(dongle.plug().vbus().await.unwrap(), VbusState::Off);

    settings
        .set_button_action(ButtonAction::CycleVbus)
        .await
        .unwrap();
    assert_eq!(
        dongle.inputs().button_action().await.unwrap(),
        ButtonAction::CycleVbus,
        "applied live"
    );
    settings.set_meter_window(ms(100)).await.unwrap();
    assert_eq!(
        dongle.meter().window().await.unwrap(),
        ms(100),
        "applied live"
    );
    let e = settings.set_meter_window(ms(5_000)).await.unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::InvalidArgument);
    let deadband = MeterDeadband {
        voltage: mv(50),
        current: Ampere::new(Prefix::Milli, 5),
    };
    settings.set_meter_deadband(deadband.clone()).await.unwrap();
    assert_eq!(settings.meter_deadband().await.unwrap(), deadband);
    let uart = settings.uart_plug().await.unwrap();
    assert_eq!(uart.baud_rate, ww_si::Baud::new(Prefix::Unit, 115_200u32));
    settings.set_uart_plug(uart).await.unwrap();

    settings.reset().await.unwrap();
    assert_eq!(
        settings.plug_vbus_at_boot().await.unwrap(),
        VbusMode::ForceOff
    );
    assert_eq!(settings.button_action().await.unwrap(), ButtonAction::Off);
    assert_eq!(mock.state().await.label, "", "reset clears the label");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leds() {
    let (mock, dongle) = start("core/leds").await;
    let leds = dongle.leds();
    leds.set_mode(ww_indication::Mode::OnlyEssential)
        .await
        .unwrap();
    assert_eq!(
        leds.mode().await.unwrap(),
        ww_indication::Mode::OnlyEssential
    );
    leds.set_brightness(ww_indication::Brightness::Lowest)
        .await
        .unwrap();
    assert_eq!(
        leds.brightness().await.unwrap(),
        ww_indication::Brightness::Lowest
    );
    leds.identify(Second::new(Prefix::Unit, 5)).await.unwrap();
    assert_eq!(mock.state().await.identified, 1);
    let e = leds
        .identify(Second::new(Prefix::Unit, 600))
        .await
        .unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::Unsupported);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn meter() {
    let (mock, dongle) = start("core/meter").await;
    let meter = dongle.meter();
    assert_eq!(meter.reading().await.unwrap().vbus_in, mv(5_000));
    mock.with(|s| {
        s.reading.current = Ampere::new(Prefix::Milli, 250);
        s.totals.charge = Coulomb::new(Prefix::Milli, 900);
    })
    .await;
    assert_eq!(
        meter.reading().await.unwrap().current,
        Ampere::new(Prefix::Milli, 250)
    );
    assert_eq!(
        meter.totals().await.unwrap().charge,
        Coulomb::new(Prefix::Milli, 900)
    );
    meter.reset_totals().await.unwrap();
    assert_eq!(
        meter.totals().await.unwrap().charge,
        Coulomb::new(Prefix::Milli, 0)
    );
    assert_eq!(
        meter.temperature().await.unwrap(),
        DegreeCelsius::new(Prefix::Unit, 30)
    );
    assert!(!meter.calibration().await.unwrap().calibrated);

    meter.set_window(ms(50)).await.unwrap();
    assert_eq!(meter.window().await.unwrap(), ms(50));
    let e = meter.set_window(ms(0)).await.unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::InvalidArgument);
    let deadband = MeterDeadband {
        voltage: mv(100),
        current: Ampere::new(Prefix::Milli, 20),
    };
    meter.set_deadband(deadband.clone()).await.unwrap();
    assert_eq!(meter.deadband().await.unwrap(), deadband);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plug_vbus_guard_and_cycle() {
    let (mock, dongle) = start("core/plug").await;
    let plug = dongle.plug();
    assert_eq!(plug.vbus().await.unwrap(), VbusState::Off);

    // vbus_cycle needs VBUS to be on by mode
    let e = plug.vbus_cycle(ms(100), false).await.unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::WrongState);

    plug.set_vbus_mode(VbusMode::ForceOn {
        max_voltage: mv(5_500),
    })
    .await
    .unwrap();
    assert_eq!(plug.vbus().await.unwrap(), VbusState::On);
    assert_eq!(plug.vbus_cycle(ms(100), true).await.unwrap(), VbusState::On);
    let state = mock.state().await;
    assert_eq!(
        (state.vbus_cycles, state.sbu_function),
        (1, SbuFunction::Off)
    );

    // a PD source went to 9 V: over the limit, the cycle refuses with the measured value
    mock.with(|s| s.reading.vbus_in = mv(9_000)).await;
    let e = plug.vbus_cycle(ms(100), false).await.unwrap_err();
    let Error::Device { item, error } = &e else {
        panic!("expected a device error, got {e:?}");
    };
    assert_eq!(item, "Plug::vbus_cycle");
    assert_eq!(error.kind, DeviceErrorKind::Specific);
    assert_eq!(
        error.typed::<PowerErrorOwned>(),
        Some(&PowerErrorOwned::OverVoltage {
            measured: mv(9_000)
        })
    );

    // switching on above the limit trips
    plug.set_vbus_mode(VbusMode::ForceOn {
        max_voltage: mv(5_500),
    })
    .await
    .unwrap();
    assert_eq!(
        plug.vbus().await.unwrap(),
        VbusState::Tripped(GuardTrip::OverVoltage)
    );
    let auto = VbusMode::Auto {
        max_voltage: mv(20_000),
        max_current: Ampere::new(Prefix::Milli, 3_000),
        max_power: Watt::new(Prefix::Milli, 60_000),
    };
    plug.set_vbus_mode(auto.clone()).await.unwrap();
    assert_eq!(plug.vbus_mode().await.unwrap(), auto);
    assert_eq!(plug.vbus().await.unwrap(), VbusState::On);
    let e = plug
        .set_vbus_mode(VbusMode::ForceOn {
            max_voltage: mv(48_000),
        })
        .await
        .unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::InvalidArgument);
    plug.set_vbus_mode(VbusMode::ForceOff).await.unwrap();
    assert_eq!(plug.vbus().await.unwrap(), VbusState::Off);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plug_data_orientation_cc() {
    let (_mock, dongle) = start("core/plug_data").await;
    let plug = dongle.plug();
    assert_eq!(plug.data().await.unwrap(), DataPath::Connected);
    plug.set_data(DataPath::Cut).await.unwrap();
    assert_eq!(plug.data().await.unwrap(), DataPath::Cut);
    plug.set_orientation(Orientation::Swapped).await.unwrap();
    assert_eq!(plug.orientation().await.unwrap(), Orientation::Swapped);
    assert_eq!(plug.cc().await.unwrap().polarity, CcPolarity::Unknown);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn header() {
    let (_mock, dongle) = start("core/header").await;
    let header = dongle.header();
    assert_eq!(header.vbus_sw().await.unwrap(), VbusState::Off);
    header
        .set_vbus_sw_mode(VbusMode::ForceOn {
            max_voltage: mv(5_500),
        })
        .await
        .unwrap();
    assert_eq!(header.vbus_sw().await.unwrap(), VbusState::On);
    assert_eq!(
        header.vbus_sw_mode().await.unwrap(),
        VbusMode::ForceOn {
            max_voltage: mv(5_500)
        }
    );
    header.set_relay(true).await.unwrap();
    assert!(header.relay().await.unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sbu() {
    let (mock, dongle) = start("core/sbu").await;
    let sbu = dongle.sbu();
    assert_eq!(sbu.function().await.unwrap(), SbuFunction::Off);
    assert_eq!(sbu.level().await.unwrap(), mv(3_300));
    sbu.set_pullups(true).await.unwrap();
    assert!(sbu.pullups().await.unwrap());

    mock.with(|s| s.sbu_function = SbuFunction::Swd).await;
    let e = sbu.set_pullups(false).await.unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::InUse);
    sbu.release().await.unwrap();
    assert_eq!(sbu.function().await.unwrap(), SbuFunction::Off);
    assert!(!sbu.pullups().await.unwrap());

    let e = sbu.route_usb(true).await.unwrap_err();
    assert_eq!(
        device_kind(e),
        DeviceErrorKind::Unsupported,
        "Main has no hub port 4"
    );
    mock.with(|s| s.variant = BoardVariant::Plus).await;
    sbu.route_usb(true).await.unwrap();
    assert_eq!(sbu.function().await.unwrap(), SbuFunction::Usb);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inputs_hub_swd() {
    let (mock, dongle) = start("core/inputs").await;
    let inputs = dongle.inputs();
    mock.with(|s| {
        s.button.presses = 3;
        s.switch = SwitchPosition::B;
    })
    .await;
    assert_eq!(inputs.button().await.unwrap().presses, 3);
    assert_eq!(inputs.switch().await.unwrap(), SwitchPosition::B);
    inputs
        .set_button_action(ButtonAction::ToggleData)
        .await
        .unwrap();
    assert_eq!(
        inputs.button_action().await.unwrap(),
        ButtonAction::ToggleData
    );

    let hub = dongle.hub();
    assert_eq!(hub.state().await.unwrap().phase, HubPhase::Attached);
    hub.recover().await.unwrap();
    let state = hub.state().await.unwrap();
    assert_eq!(
        (state.recoveries, state.last_cause),
        (1, Some(HubLoss::Requested))
    );

    let swd = dongle.swd();
    assert_eq!(swd.mode().await.unwrap(), SwdMode::BitBang);
    swd.set_mode(SwdMode::Spi).await.unwrap();
    assert_eq!(swd.mode().await.unwrap(), SwdMode::Spi);
    assert_eq!(swd.session().await.unwrap(), SwdSession::Idle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn io_levels_unimplemented() {
    let (_mock, dongle) = start("core/io").await;
    // the arrays have their shape, the items answer Unimplemented until their pieces land
    let indices = dongle.client().uart_valid_indices().read().await.unwrap();
    assert!(
        format!("{indices:?}").contains('2'),
        "uart[0..2]: {indices:?}"
    );
    for side in [Side::Plug, Side::Header] {
        unimplemented(dongle.uart(side).capabilities().await);
        unimplemented(dongle.i2c(side).scan().await);
        unimplemented(dongle.gpio(side).name().await);
        unimplemented(dongle.gpio(side).input_level(0).await);
    }
    unimplemented(dongle.adc().read(0).await);
    unimplemented(dongle.adc().capabilities().await);
    let e = dongle.i2c(Side::Plug).scan().await.unwrap_err();
    assert!(e.to_string().contains("not implemented"), "{e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn debug() {
    let (mock, dongle) = start("core/debug").await;
    let debug = dongle.debug();
    let relay_en = donguru_core::nets::pin("RELAY_EN").expect("RELAY_EN in the table");
    let state = debug
        .pin_set(relay_en, PinMode::PushPull, Some(Level::High))
        .await
        .unwrap();
    assert_eq!((state.moder, state.output), (1, true));
    assert_eq!(debug.pin_get(relay_en).await.unwrap(), state);
    let state = debug.pin_af(relay_en, 4).await.unwrap();
    assert_eq!((state.moder, state.af), (2, 4));

    for refused in ["NRST", "USB_DP", "SWDIO", "PWR_EN"] {
        let pin =
            donguru_core::nets::pin(refused).unwrap_or_else(|| panic!("{refused} in the table"));
        let e = debug.pin_set(pin, PinMode::Input, None).await.unwrap_err();
        assert_eq!(device_kind(e), DeviceErrorKind::Refused, "{refused}");
    }
    let bad = McuPin {
        port: GpioPort::B,
        pin: 16,
    };
    assert_eq!(
        device_kind(debug.pin_get(bad).await.unwrap_err()),
        DeviceErrorKind::InvalidArgument
    );

    let pins = debug.pins().await.unwrap();
    assert_eq!(pins.ports.len(), 6);
    let a = &pins.ports[0];
    assert_eq!(
        (a.port, (a.moder >> 10) & 3, (a.odr >> 5) & 1),
        (GpioPort::A, 2, 1),
        "PA5 in the dump"
    );
    let boot = debug.boot_pins().await.unwrap();
    assert_eq!(boot.ports[0].moder, u32::MAX, "all analog at reset");

    assert_eq!(
        debug.hub_read(0x10, 4).await.unwrap(),
        vec![0x10, 0x11, 0x12, 0x13]
    );
    debug.hub_write(0x10, &[0xAA, 0xBB]).await.unwrap();
    assert_eq!(debug.hub_read(0x10, 2).await.unwrap(), vec![0xAA, 0xBB]);
    let e = debug.hub_read(0xFF, 8).await.unwrap_err();
    let Error::Device { error, .. } = &e else {
        panic!("{e:?}")
    };
    assert_eq!(
        error.typed::<DebugErrorOwned>(),
        Some(&DebugErrorOwned::Nack)
    );
    assert!(
        debug.hub_write(0, &[0; 33]).await.is_err(),
        "at most 32 bytes"
    );

    let e = debug
        .ucpd_role(UcpdPort::Ucpd1, CcRole::Rp3A0)
        .await
        .unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::Refused);
    assert_eq!(
        debug
            .ucpd_role(UcpdPort::Ucpd2, CcRole::Rd)
            .await
            .unwrap()
            .cr,
        1
    );
    assert_eq!(debug.ucpd(UcpdPort::Ucpd2).await.unwrap().cr, 1);

    debug
        .led_set(
            Led::Status,
            LedMode::Blink {
                on_ms: 100,
                off_ms: 400,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        mock.state().await.leds[1],
        LedMode::Blink {
            on_ms: 100,
            off_ms: 400
        }
    );

    assert_eq!(
        debug.reg_read(0x4800_0000, 2).await.unwrap(),
        vec![0x4800_0000, 0x4800_0004]
    );
    let e = debug.reg_read(0x2000_0000, 1).await.unwrap_err();
    assert_eq!(
        device_kind(e),
        DeviceErrorKind::Refused,
        "RAM is not peripheral space"
    );
    let e = debug.reg_read(0x4000_0000, 33).await.unwrap_err();
    assert_eq!(device_kind(e), DeviceErrorKind::InvalidArgument);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observe_plug_and_meter() {
    let (mock, dongle) = start("core/observe").await;
    let plug = dongle
        .plug()
        .observe(PlugObserve::default().vbus(on_change(Duration::ZERO)))
        .await;
    let mut plug = plug.expect("observe plug.vbus");
    dongle
        .plug()
        .set_vbus_mode(VbusMode::ForceOn {
            max_voltage: mv(5_500),
        })
        .await
        .unwrap();
    let got = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let update = plug.recv().await.expect("plug update");
            if update.value.vbus == Some(VbusState::On) {
                return;
            }
        }
    })
    .await;
    assert!(got.is_ok(), "no plug.vbus = On update");

    let mut meter = dongle
        .meter()
        .observe(PowerMeterObserve::default().reading(on_change(Duration::ZERO)))
        .await
        .expect("observe meter.reading");
    mock.with(|s| s.reading.vbus_in = mv(5_100)).await;
    let got = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let update = meter.recv().await.expect("meter update");
            if update
                .value
                .reading
                .as_ref()
                .is_some_and(|r| r.vbus_in == mv(5_100))
            {
                return;
            }
        }
    })
    .await;
    assert!(got.is_ok(), "no meter.reading update");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stuck_device_times_out() {
    let (mock, dongle) = start("core/stuck").await;
    mock.drop_requests(true);
    let e = dongle.plug().vbus().await.unwrap_err();
    assert!(matches!(e, Error::Timeout(_)), "{e:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_missing_in_process_device() {
    let entry = donguru_core::DeviceEntry::in_process("core/nobody");
    let Err(e) = Dongle::open_entry(entry).await else {
        panic!("opened a device that isn't there");
    };
    assert!(matches!(e, Error::NoDevice(_)), "{e:?}");
}
