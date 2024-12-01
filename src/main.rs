use std::{
    cell::LazyCell,
    sync::{Arc, LazyLock},
    time::Duration,
};

use esp_idf_hal::{
    delay::FreeRtos,
    peripheral::Peripheral,
    prelude::Peripherals,
    rmt::{
        config::{CarrierConfig, DutyPercent},
        FixedLengthSignal, PinState, Pulse, PulseTicks, Receive, RmtReceiveConfig,
        RmtTransmitConfig, RxRmtDriver, Signal, TxRmtDriver, VariableLengthSignal,
    },
    units::Hertz,
};
use packet::{Action, Channel, Packet};
use std::sync::Mutex;
mod packet;

struct Tx {
    pub tx: TxRmtDriver<'static>,
    pub sync_high: Pulse,
    pub sync_low: Pulse,
    pub one_high: Pulse,
    pub one_low: Pulse,
    pub zero_high: Pulse,
    pub zero_low: Pulse,
}

static PERIPHERALS: LazyLock<Mutex<Peripherals>> =
    LazyLock::new(|| Mutex::new(Peripherals::take().unwrap()));

static TX: LazyLock<Mutex<Tx>> = LazyLock::new(|| {
    let mut peripherals = PERIPHERALS.lock().unwrap();

    let mut config = RmtTransmitConfig::new();
    log::info!("Configuring RMT {:?}", config);

    let carrier_config = CarrierConfig::new()
        // .frequency(Hertz(1000))
        .frequency(Hertz(1_000_000))
        .duty_percent(DutyPercent::new(50).unwrap())
        .carrier_level(PinState::High);

    config = config
        .carrier(Some(carrier_config))
        .clock_divider(10)
        .idle(Some(PinState::Low));

    let tx = TxRmtDriver::new(
        unsafe { peripherals.rmt.channel1.clone_unchecked() },
        unsafe { peripherals.pins.gpio0.clone_unchecked() },
        &config,
    )
    .unwrap();

    log::info!("Configuring RMT {:?}", config);

    log::info!("Hello, world!");

    let ticks_hz = tx.counter_clock().unwrap();

    log::info!("tickshz {:?}", ticks_hz);

    let sync_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(1400)).unwrap();
    let sync_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(750)).unwrap();
    let one_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(750)).unwrap();
    let one_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(250)).unwrap();
    let zero_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(250)).unwrap();
    let zero_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(750)).unwrap();

    return Mutex::new(Tx {
        tx,
        sync_high,
        sync_low,
        one_high,
        one_low,
        zero_high,
        zero_low,
    });
});

impl Tx {
    fn send_stream(&mut self, data: &[bool]) {
        let mut signal = VariableLengthSignal::new();
        signal.push(&[self.sync_high, self.sync_low]);

        for (_, bit) in data.iter().enumerate() {
            if *bit {
                signal.push(&[self.one_high, self.one_low]);
            } else {
                signal.push(&[self.zero_high, self.zero_low]);
            }
        }
        self.tx.start(signal).unwrap();
    }

    fn send_packet(&mut self, packet: &Packet) {
        let mut signal = FixedLengthSignal::<{ 1 + (8 * 6) + 2 }>::new();
        signal.set(0, &(self.sync_high, self.sync_low)).unwrap();

        let bits = packet.to_bits();

        for (index, bit) in bits.iter().enumerate() {
            if *bit {
                signal
                    .set(1 + index, &(self.one_high, self.one_low))
                    .unwrap();
            } else {
                signal
                    .set(1 + index, &(self.zero_high, self.zero_low))
                    .unwrap();
            }
        }
        self.tx.start(signal).unwrap();
    }
}

fn main() {
    // unsafe {
    //     esp_idf_sys::sleep(2);
    // }
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    loop {
        let packet = Packet {
            id: 0b1111010101010000,
            channel: Channel::Zero,
            action: Action::Vibrate,
            intensity: 0b10101010,
        };
        TX.lock().unwrap().send_packet(&packet);
        FreeRtos::delay_ms(50);
    }
}
