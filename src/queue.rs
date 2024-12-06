use esp_idf_hal::{
    peripheral::Peripheral,
    prelude::Peripherals,
    rmt::{FixedLengthSignal, PinState, Pulse, RmtTransmitConfig, TxRmtDriver},
};
use esp_idf_sys::rmt_register_tx_end_callback;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender},
    },
    time::Duration,
};
use {
    esp_idf_sys::{esp, esp_vfs_dev_uart_use_driver, uart_driver_install},
    std::ptr::null_mut,
};

/// Atomic boolean tracking whether the transmitter is currently transmitting.
static TRANSMITTING: AtomicBool = AtomicBool::new(false);

/// Callback for when the transmitter finishes transmitting.
extern "C" fn transmit_finish(_channel: u32, _arg: *mut std::ffi::c_void) {
    TRANSMITTING.store(false, Ordering::Relaxed);
}

/// Queue struct
pub struct Queue {
    /// Transmitter queue
    tx: Sender<Vec<bool>>,
    rx: Receiver<Vec<bool>>,
    /// Driver for the transmitter
    driver: TxRmtDriver<'static>,
    /// Pulse encoder
    pulses: Pulses,
}

impl Queue {
    /// Create a new queue.
    fn new() -> Self {
        // create channels
        let (tx, rx) = std::sync::mpsc::channel();

        // create the transmitter
        let mut config = RmtTransmitConfig::new();
        config = config
            .carrier(None)
            .clock_divider(10)
            .idle(Some(PinState::Low));

        let mut peripherals = Peripherals::take().unwrap();
        let driver = TxRmtDriver::new(
            unsafe { peripherals.rmt.channel1.clone_unchecked() },
            unsafe { peripherals.pins.gpio0.clone_unchecked() },
            &config,
        )
        .unwrap();

        // create the pulse encoder
        let pulses = Pulses::new(&driver);

        Self {
            tx,
            rx,
            driver,
            pulses,
        }
    }

    /// Send a packet.
    pub fn send(&self, packet: Vec<bool>) {
        self.tx.send(packet).unwrap();
    }

    /// Tick the transmitter.
    pub fn tick(&mut self) {
        // skip if transmitting
        if TRANSMITTING.load(Ordering::Relaxed) {
            return;
        }

        // get the next packet
        let Ok(packet) = self.rx.try_recv() else {
            return;
        };

        // transmit the packet
        let signal = self.pulses.encode_bits(&packet);
        TRANSMITTING.store(true, Ordering::Relaxed);
        self.driver.start(signal).unwrap();
    }
}

/// Pulses used to encode bits
struct Pulses {
    sync_high: Pulse,
    sync_low: Pulse,
    one_high: Pulse,
    one_low: Pulse,
    zero_high: Pulse,
    zero_low: Pulse,
}

impl Pulses {
    /// Create a new set of pulses
    fn new(driver: &TxRmtDriver) -> Pulses {
        let ticks_hz = driver.counter_clock().unwrap();

        let create_pulse = |state, duration| {
            Pulse::new_with_duration(ticks_hz, state, &Duration::from_micros(duration)).unwrap()
        };

        Pulses {
            sync_high: create_pulse(PinState::High, 1400),
            sync_low: create_pulse(PinState::Low, 800),
            one_high: create_pulse(PinState::High, 800),
            one_low: create_pulse(PinState::Low, 300),
            zero_high: create_pulse(PinState::High, 300),
            zero_low: create_pulse(PinState::Low, 800),
        }
    }
    /// Encode a vector of bits into a signal
    fn encode_bits(&self, bits: &Vec<bool>) -> FixedLengthSignal<{ 1 + (8 * 5) + 3 }> {
        let mut signal = FixedLengthSignal::<{ 1 + (8 * 5) + 3 }>::new();
        signal.set(0, &(self.sync_high, self.sync_low)).unwrap();
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
        return signal;
    }
}

/// Initialize the transmitter.
pub unsafe fn init() -> Queue {
    // setup the peripherals
    esp_idf_svc::sys::link_patches();

    esp!(uart_driver_install(0, 512, 512, 10, null_mut(), 0)).unwrap();
    esp_vfs_dev_uart_use_driver(0);

    // setup the logger
    esp_idf_svc::log::EspLogger::initialize_default();

    // register the transmit finish callback
    rmt_register_tx_end_callback(Some(transmit_finish), null_mut());

    // create queues for the transmitter
    Queue::new()
}
