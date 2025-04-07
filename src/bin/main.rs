#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]

use core::cell::OnceCell;

use defmt::info;
use embassy_executor::Spawner;
use embassy_net::HardwareAddress;
use embassy_sync::{blocking_mutex::NoopMutex, blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal_embassy::main;
use esp_wifi::{
    EspWifiController,
    esp_now::{
        self, BROADCAST_ADDRESS, EspNow, EspNowManager, EspNowReceiver, EspNowSender,
        EspNowWithWifiCreateToken,
    },
    init,
    wifi::{
        self, AuthMethod, ClientConfiguration, Configuration, RxControlInfo, WifiController,
        WifiDevice, ap_mac, sta_mac,
    },
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
const WIFI_SSID: &str = "P601";
const WIFI_PASSWORD: &str = "00000000";
const WIFI_CHANNEL: Option<u8> = Some(10);

type SharedWifiController<'a> = &'a Mutex<NoopRawMutex, WifiController<'a>>;

#[main]
async fn main(_spawner: Spawner) {
    // generator version: 0.3.1

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0: SystemTimer = SystemTimer::new(peripherals.SYSTIMER);
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

    let (wifi, esp_now_with_wifi_token) = esp_now::enable_esp_now_with_wifi(peripherals.WIFI);

    let (mut wifi_controller, wifi_interface) = wifi::new(&esp_wifi_ctrl, wifi).unwrap();
    let wifi_config = Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        auth_method: AuthMethod::WPAWPA2Personal,
        password: WIFI_PASSWORD.try_into().unwrap(),
        channel: WIFI_CHANNEL,
        ..Default::default()
    });

    let _ = WifiController::set_configuration(&mut wifi_controller, &wifi_config)
        .expect("Can't set configuration for Wifi Controller");
    let _ = wifi_controller
        .start()
        .expect("Can't start Wifi Controller");
    let _ = wifi_controller.connect().expect("Something is wrong!!!");
    // let wifi_controller = mk_static!(WifiController<'static>, wifi_controller);
    let mac_address_bytes: [u8; 6] = wifi_interface.sta.mac_address();
    info!(
        "Device MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac_address_bytes[0],
        mac_address_bytes[1],
        mac_address_bytes[2],
        mac_address_bytes[3],
        mac_address_bytes[4],
        mac_address_bytes[5]
    );
    let wifi_controller: SharedWifiController<'static> =
        mk_static!(Mutex<NoopRawMutex, WifiController<'static>>, Mutex::new(wifi_controller));
    info!("WIFI state: {}", wifi::wifi_state());

    let esp_now = EspNow::new_with_wifi(&esp_wifi_ctrl, esp_now_with_wifi_token).unwrap();
    let _ = esp_now.set_channel(WIFI_CHANNEL.unwrap());
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
    let _ = _spawner.spawn(wifireporter(wifi_controller));

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
        info!("Sending BROADCAST...");
        let mut sender = sender.lock().await;
        let status = sender.send_async(&BROADCAST_ADDRESS, b"Hello").await;
        info!("Send BROADCAST: {:?}", status);
    }
}

#[embassy_executor::task]
async fn wifireporter(wifi_controller: SharedWifiController<'static>) {
    loop {
        Timer::after(Duration::from_secs(1)).await;
        info!(
            "WIFI is connected?: {}",
            wifi_controller.lock().await.is_connected()
        );
    }
}
