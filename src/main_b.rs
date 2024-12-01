use std::{sync::Arc, time::Duration};

use esp32_nimble::utilities::mutex::Mutex;
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
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

    log::info!("Hello, world!");

    let peripherals = Peripherals::take().unwrap();

    // tx.set_looping(esp_idf_hal::rmt::config::Loop::None)
    //     .unwrap();
    let packet = Packet {
        id: 0b1111010101010000,
        channel: Channel::Zero,
        action: Action::Vibrate,
        intensity: 0b10101010,
    };
    let bits = packet.to_bits();

    let mut pin = PinDriver::output(peripherals.pins.gpio0).unwrap();
    // pin.set_pull(Pull::Down).unwrap();
    pin.set_drive_strength(esp_idf_hal::gpio::DriveStrength::I40mA)
        .unwrap();

    // BaseClock for the Timer is the APB_CLK that is running on 80MHz at default
    // The default clock-divider is -> 80
    // default APB clk is available with the APB_CLK_FREQ constant
    let timer_conf = esp_idf_hal::timer::config::Config::new().auto_reload(true);
    let mut timer = TimerDriver::new(peripherals.timer00, &timer_conf).unwrap();

    // Every half a second
    timer.set_alarm(timer.tick_hz() / 20000).unwrap();
    log::info!("Timer Configured with a frequency of {:?}", timer.tick_hz());

    let mut pin_state = Box::new(0u32);
    let mut pin_state_ptr = pin_state.as_mut() as *mut u32;
    // Saftey: make sure the `Notification` object is not dropped while the subscription is active
    unsafe {
        timer
            .subscribe(move || unsafe {
                let mut state = PIN_STATE;
                PIN_STATE += 1;
                match state {
                    5 => {
                        pin.set_low().unwrap();
                    }
                    20 => {
                        pin.set_high().unwrap();
                        PIN_STATE = 0;
                    }
                    _ => {}
                }
            })
            .unwrap();
    }

    timer.enable_interrupt().unwrap();
    timer.enable_alarm(true).unwrap();
    timer.enable(true).unwrap();

    loop {
        // let mut signal = FixedLengthSignal::<{ 1 + (8 * 6) + 2 }>::new();
        // signal.set(0, &(sync_high, sync_low)).unwrap();

        // for (index, bit) in bits.iter().enumerate() {
        //     if *bit {
        //         signal.set(1 + index, &(one_high, one_low)).unwrap();
        //     } else {
        //         signal.set(1 + index, &(zero_high, zero_low)).unwrap();
        //     }
        // }

        // tx.start_blocking(&signal).unwrap();

        FreeRtos::delay_ms(200);
        // pin.set_low().unwrap();
        // FreeRtos::delay_ms(200);

        // println!("Tx Loop");

        // Transmit the signal (send sequence)
    }
}
