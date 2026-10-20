//! CPU load (FW-43): the thread-mode executor, run by hand to count the time the core sleeps in the counter
//! `idle_us`. The load over a span is 1 - (idle_us difference) / (span in us): `device.counters` read twice.
//!
//! Embassy's own `Executor::run` is `poll(); wfe` in a loop; this one masks interrupts around the WFE and reads the
//! clock on both sides, so the sleep is measured without the interrupt handlers that end it (they run once
//! interrupts are on again, and count as busy). SEVONPEND lets an interrupt that becomes pending wake the WFE while
//! masked; a task woken from thread mode or a handler sets the event register with SEV (the pender) as before.

use cnt::cnt;
use embassy_executor::{Spawner, raw};
use embassy_time::Instant;
use static_cell::StaticCell;

static EXECUTOR: StaticCell<raw::Executor> = StaticCell::new();

/// Runs the thread-mode executor with the tasks `init` spawns, never returns.
pub fn run(init: impl FnOnce(Spawner)) -> ! {
    // usize::MAX: the context embassy-executor's Cortex-M pender takes for the thread-mode executor (SEV)
    let executor = EXECUTOR.init(raw::Executor::new(usize::MAX as *mut ()));
    // Safety: SCR is only written here, before any task runs
    unsafe { cortex_m::Peripherals::steal().SCB.set_sevonpend() };
    init(executor.spawner());
    loop {
        // Safety: polled from this one context only
        unsafe { executor.poll() };
        // wake-ups of the executor: many per second with little idle time means a task that keeps waking itself
        cnt!(executor_polls: u32);
        cortex_m::interrupt::disable();
        let sleep = Instant::now();
        cortex_m::asm::wfe();
        let slept = sleep.elapsed().as_micros();
        // Safety: not inside a critical section, interrupts were on before the disable above
        unsafe { cortex_m::interrupt::enable() };
        cnt!(idle_us: u64 "us" += slept);
    }
}
