use std::{sync::Arc, time::Duration};

use esp32_nimble::utilities::mutex::Mutex;
use esp_idf_hal::adc::attenuation::DB_11;
use esp_idf_hal::adc::oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver};
use esp_idf_hal::adc::Resolution;
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    peripherals,
    prelude::Peripherals,
    rmt::{
        config::{CarrierConfig, DutyPercent},
        FixedLengthSignal, PinState, Pulse, PulseTicks, RmtTransmitConfig, Signal, TxRmtDriver,
        VariableLengthSignal,
    },
    timer::TimerDriver,
    units::Hertz,
};
use packet::{Action, Channel, Packet};
mod packet;

static mut PIN_STATE: u32 = 0;

fn main() {
    // unsafe {
    //     esp_idf_sys::sleep(2);
    // }
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();

    let mut adc = AdcDriver::new(peripherals.adc1).unwrap();

    let config = AdcChannelConfig {
        attenuation: DB_11,
        resolution: Resolution::Resolution12Bit,
        calibration: false,
    };
    let mut adc_pin = AdcChannelDriver::new(&adc, peripherals.pins.gpio1, &config).unwrap();
    loop {
        // you can change the sleep duration depending on how often you want to sample
        // thread::sleep(Duration::from_millis(10));
        // for _ in 0..1000 {
        // you can change the sleep duration depending on how often you want to sample
        let mut values = [0u32; 500];
        for i in 0..10 {
            let value = adc.read(&mut adc_pin).unwrap();
            values[i] = value as u32;
        }
        let average = values.iter().sum::<u32>() / values.len() as u32;

        let dots = ".".repeat((average / 5) as usize);
        println!("ADC value: {} {}", average, dots);
        // }
        // FreeRtos::delay_ms(10);
    }
}
