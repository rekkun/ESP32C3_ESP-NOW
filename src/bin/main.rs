#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]

use defmt::info;
use embassy_executor::{Executor, Spawner};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, blocking_mutex::NoopMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_wifi::{
    esp_now::{EspNow, EspNowManager, EspNowReceiver, EspNowSender, BROADCAST_ADDRESS},
    init, EspWifiController,
};
use panic_rtt_target as _;
use static_cell::StaticCell;

extern crate alloc;

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

// static EXECUTOR: StaticCell<Executor> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // generator version: 0.3.1

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Embassy initialized!");

    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        init(
            timer1.timer0,
            esp_hal::rng::Rng::new(peripherals.RNG),
            peripherals.RADIO_CLK,
        )
        .unwrap()
    );

    let esp_now = EspNow::new(&esp_wifi_ctrl, peripherals.WIFI).unwrap();
    let _ = esp_now.set_channel(3);
    info!("ESP_NOW version {}", esp_now.version().unwrap());
    let (manager, sender, receiver) = esp_now.split();
    let manager = mk_static!(EspNowManager, manager);
    let sender = mk_static!(
        Mutex::<NoopRawMutex, EspNowSender<'static>>,
        Mutex::<NoopRawMutex, _>::new(sender)
    );
    // TODO: Spawn some tasks
    //let _ = spawner;
    // let executor = EXECUTOR.init(Executor::new());

    // executor.run(|_spawner| {
    //     info!("Spawning tasks...");
    //     _spawner
    //         .spawn(broadcaster(sender))
    //         .expect("Failed to spawn broadcaster task");
    //     info!("Task spawned");
    // });
    let _ = _spawner.spawn(broadcaster(sender));

    // loop {
    //     info!("Hello world!");
    //     Timer::after(Duration::from_secs(1)).await;
    // }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-beta.0/examples/src/bin
}

#[embassy_executor::task]
async fn broadcaster(sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>) {
    loop {
        Timer::after(Duration::from_secs(1)).await;
        info!("Dang gui BROADCAST...");
        let mut sender = sender.lock().await;
        let status = sender.send_async(&BROADCAST_ADDRESS, b"Hello").await;
        info!("Gui BROADCAST: {:?}", status);
    }
}
