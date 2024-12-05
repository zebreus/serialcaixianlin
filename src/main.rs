#![feature(str_from_raw_parts)]
#![feature(lazy_get)]
use esp_idf_hal::{
    delay::FreeRtos,
    peripheral::Peripheral,
    prelude::Peripherals,
    rmt::{FixedLengthSignal, PinState, Pulse, RmtTransmitConfig, TxRmtDriver},
};
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, EspNvsPartition, NvsDefault};
use esp_idf_sys::rmt_register_tx_end_callback;
use packet::{Action, Channel, Packet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, Sender},
    Mutex, RwLock,
};
use std::{sync::LazyLock, time::Duration};
use {
    esp_idf_sys::{esp, esp_vfs_dev_uart_use_driver, uart_driver_install},
    std::ptr::null_mut,
};
mod packet;

struct Config {
    pub id: u16,
    pub channel: Channel,
    pub intensity: u8,
    pub action: Action,
    nvs: EspNvs<NvsDefault>,
}
static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| {
    let nvs_default_partition: EspNvsPartition<NvsDefault> =
        EspDefaultNvsPartition::take().unwrap();
    let nvs = EspNvs::new(nvs_default_partition, "sc_config", true).unwrap();

    let id = nvs.get_u16("id").unwrap_or(Some(0)).unwrap_or(0);
    let intensity = nvs.get_u8("intensity").unwrap_or(Some(3)).unwrap_or(3);
    let action = nvs.get_u8("action").unwrap_or(Some(1)).unwrap_or(1);
    let channel = nvs.get_u8("channel").unwrap_or(Some(0)).unwrap_or(0);
    if channel > 3 {
        nvs.set_u16("channel", 0).unwrap();
        panic!("Channel must be between 0 and 3");
    }
    if action == 0 || action > 3 {
        nvs.set_u16("action", 3).unwrap();
        panic!("Action must be between 1 and 3");
    }

    let channel = Channel::from(channel);
    let action = Action::from(action);

    RwLock::new(Config {
        id,
        channel,
        intensity,
        action,
        nvs,
    })
});

static SENDING: AtomicBool = AtomicBool::new(false);
extern "C" fn tx_complete_callback(_channel: u32, _arg: *mut std::ffi::c_void) {
    SENDING.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn print_help() {
    // Light may go up to 255
    println!(
        r#"Available commands:
    help              : Print this help page
    id 0-65535        : Set the id of this transmitter
    channel 0-2       : Set the channel of this transmitter
    intensity 0-99    : Set the intensity of the command
    shock [0-99]      : Set the command type to zapping with the given intensity
    vibrate [0-99]    : Set the command type to good vibrations with the given
                        intensity
    beep              : Set the command type to make beepy noises
    light [0-99]      : Set the command type to enable the light. Light is
                        toggled when a light command with a different intensity
                        then the last light command is sent
    transmit [1-1000] : Transmit the configured command the given amount (default 4)
    find [1-200]      : Make noise and flash for a given number of flashes (default 10)
    abort             : Stop any ongoing transmission
    "#
    );
}
fn process_command(command: String, queue_packet: &Sender<Packet>, abort: &Receiver<Packet>) {
    let mut split_command = command.split(" ");
    let args = (
        split_command.next().unwrap_or(""),
        split_command
            .next()
            .unwrap_or("-1")
            .parse::<i32>()
            .unwrap_or(-1),
    );
    match args {
        ("help", _) => {
            print_help();
        }
        ("id", id) => {
            if !(0..=65535).contains(&id) {
                println!("ID must be between 0 and 65535");
                return;
            }
            let mut config = CONFIG.write().unwrap();
            config.id = id as u16;
            config.nvs.set_u16("id", id as u16).unwrap();
            println!("Setting ID to {}", id);
        }
        ("channel" | "c", channel) => {
            if !(0..=2).contains(&channel) {
                println!("Channel must be between 0 and 2");
                return;
            }
            let mut config = CONFIG.write().unwrap();
            config.channel = Channel::from(channel as u8);
            config.nvs.set_u8("channel", channel as u8).unwrap();
            println!("Setting channel to {}", channel);
        }
        ("intensity" | "i", intensity) => {
            if intensity < 0 {
                println!("Intensities lower than 0 are considered 0");
            }
            if intensity >= 100 {
                println!("Intensities greater than 99 are clamped to 99");
            }
            // Intensities below 0 and above 99 do nothing, so we clamp them
            let capped_intensity = intensity.clamp(0, 99) as u8;
            let mut config = CONFIG.write().unwrap();
            if capped_intensity != config.intensity {
                config.intensity = capped_intensity;
                config.nvs.set_u8("intensity", capped_intensity).unwrap();
            }
            println!("Setting intensity to {}", capped_intensity);
        }
        ("vibrate" | "v", intensity) => {
            let mut config = CONFIG.write().unwrap();
            if intensity != -1 {
                if intensity < 0 {
                    println!("Intensities lower than 0 are considered 0");
                }
                if intensity >= 100 {
                    println!("Intensities greater than 99 are clamped to 99");
                }
                // Intensities below 0 and above 99 do nothing, so we clamp them
                let capped_intensity = intensity.clamp(0, 99) as u8;
                if capped_intensity != config.intensity {
                    config.intensity = capped_intensity;
                    config.nvs.set_u8("intensity", capped_intensity).unwrap();
                }
            }
            config.action = Action::Vibrate;
            config.nvs.set_u8("action", Action::Vibrate as u8).unwrap();
            println!("Setting action to vibrate {}", config.intensity);
        }
        ("shock" | "s", intensity) => {
            let mut config = CONFIG.write().unwrap();
            if intensity != -1 {
                if intensity < 0 {
                    println!("Intensities lower than 0 are considered 0");
                }
                if intensity >= 100 {
                    println!("Intensities greater than 99 are clamped to 99");
                }
                // Intensities below 0 and above 99 do nothing, so we clamp them
                let capped_intensity = intensity.clamp(0, 99) as u8;
                if capped_intensity != config.intensity {
                    config.intensity = capped_intensity;
                    config.nvs.set_u8("intensity", capped_intensity).unwrap();
                }
            }
            config.action = Action::Shock;
            config.nvs.set_u8("action", Action::Shock as u8).unwrap();
            println!("Setting action to shock {}", config.intensity);
        }
        ("beep" | "b", _) => {
            let mut config = CONFIG.write().unwrap();
            config.action = Action::Beep;
            config.nvs.set_u8("action", Action::Beep as u8).unwrap();
            println!("Setting action to beep");
        }
        ("light" | "l", intensity) => {
            let mut config = CONFIG.write().unwrap();
            if intensity != -1 {
                if intensity < 0 {
                    println!("Intensities lower than 0 are considered 0");
                }
                if intensity >= 100 {
                    println!("Intensities greater than 99 are clamped to 99");
                }
                // Intensities below 0 and above 99 do nothing, so we clamp them
                let capped_intensity = intensity.clamp(0, 99) as u8;
                if capped_intensity != config.intensity {
                    config.intensity = capped_intensity;
                    config.nvs.set_u8("intensity", capped_intensity).unwrap();
                }
            }
            config.action = Action::Light;
            config.nvs.set_u8("action", Action::Light as u8).unwrap();
            println!("Setting action to light {}", config.intensity);
        }
        ("transmit" | "t", amount) => {
            let amount = if amount == -1 { 4 } else { amount };
            if amount < 0 || amount > 65535 {
                println!("Amount must be between 0 and 65535");
                return;
            }
            let config = CONFIG.read().unwrap();

            let packet = Packet {
                id: config.id,
                channel: config.channel,
                action: config.action,
                intensity: config.intensity,
            };

            println!("Transmitting packet {:?}", packet);

            for _ in 0..(amount.saturating_sub(1)) {
                queue_packet.send(packet.clone()).unwrap();
            }
            if amount != 0 {
                queue_packet.send(packet).unwrap();
            }
        }
        ("find", amount) => {
            let amount = if amount == -1 { 10 } else { amount };
            if amount < 0 || amount > 200 {
                println!("Find beeps must be between 0 and 200");
                return;
            }
            let config = CONFIG.read().unwrap();
            for _ in 0..amount {
                for _ in 0..3 {
                    let packet = Packet {
                        id: config.id,
                        channel: config.channel,
                        action: Action::Light,
                        intensity: 1,
                    };
                    queue_packet.send(packet).unwrap();
                }
                for _ in 0..4 {
                    let packet = Packet {
                        id: config.id,
                        channel: config.channel,
                        action: Action::Beep,
                        intensity: 0,
                    };
                    queue_packet.send(packet).unwrap();
                }
                for _ in 0..3 {
                    let packet = Packet {
                        id: config.id,
                        channel: config.channel,
                        action: Action::Light,
                        intensity: 0,
                    };
                    queue_packet.send(packet).unwrap();
                }
                for _ in 0..4 {
                    let packet = Packet {
                        id: config.id,
                        channel: config.channel,
                        action: Action::Beep,
                        intensity: 0,
                    };
                    queue_packet.send(packet).unwrap();
                }
            }

            let packet = Packet {
                id: config.id,
                channel: config.channel,
                action: config.action,
                intensity: config.intensity,
            };

            println!("Transmitting packet {:?}", packet);

            for _ in 0..(amount.saturating_sub(1)) {
                queue_packet.send(packet.clone()).unwrap();
            }
            if amount != 0 {
                queue_packet.send(packet).unwrap();
            }
        }
        ("abort" | "a", _) => {
            println!("Aborting transmission");
            // Drain the receiver
            while abort.try_recv().is_ok() {}
        }
        _ => {
            println!("Unknown command {}", command);
            print_help();
        }
    }
}

static TX: LazyLock<Mutex<TxRmtDriver<'static>>> = LazyLock::new(|| {
    let mut peripherals = Peripherals::take().unwrap();

    let mut config = RmtTransmitConfig::new();

    config = config
        .carrier(None)
        .clock_divider(10)
        .idle(Some(PinState::Low));

    let tx = TxRmtDriver::new(
        unsafe { peripherals.rmt.channel1.clone_unchecked() },
        unsafe { peripherals.pins.gpio0.clone_unchecked() },
        &config,
    )
    .unwrap();

    Mutex::new(tx)
});

struct Pulses {
    pub sync_high: Pulse,
    pub sync_low: Pulse,
    pub one_high: Pulse,
    pub one_low: Pulse,
    pub zero_high: Pulse,
    pub zero_low: Pulse,
}

static PULSES: LazyLock<Pulses> = LazyLock::new(|| {
    let ticks_hz = TX.lock().unwrap().counter_clock().unwrap();

    let sync_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(1400)).unwrap();
    let sync_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(800)).unwrap();

    let one_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(800)).unwrap();
    let one_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(300)).unwrap();

    let zero_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(300)).unwrap();
    let zero_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(800)).unwrap();

    Pulses {
        sync_high,
        sync_low,
        one_high,
        one_low,
        zero_high,
        zero_low,
    }
});

fn packet_to_signal(packet: &Packet) -> FixedLengthSignal<{ 1 + (8 * 5) + 3 }> {
    let mut signal = FixedLengthSignal::<{ 1 + (8 * 5) + 3 }>::new();
    signal.set(0, &(PULSES.sync_high, PULSES.sync_low)).unwrap();
    let bits = packet.to_bits();
    for (index, bit) in bits.iter().enumerate() {
        if *bit {
            signal
                .set(1 + index, &(PULSES.one_high, PULSES.one_low))
                .unwrap();
        } else {
            signal
                .set(1 + index, &(PULSES.zero_high, PULSES.zero_low))
                .unwrap();
        }
    }
    return signal;
}

fn interactive() {
    let mut buffer = String::new();

    LazyLock::force(&CONFIG);

    let (tx, rx) = std::sync::mpsc::channel();
    // Start the tx loop

    unsafe {
        rmt_register_tx_end_callback(Some(tx_complete_callback), null_mut());
    }
    loop {
        // This is the only reliable way to read a single character from stdin I found
        let next = unsafe { libc::getchar() };
        if next != -1 {
            let next_byte = char::from(next as u8);
            if buffer.len() >= 100 {
                // https://mozz.us/ascii-art/2023-05-01/longcat.html
                println!(
                    r#"Your command is too looooooooooong
                           _     
                 __       / |     
                 \ "-..--'_4|_     
      _._____     \ _  _(C "._'._     
     ((^     '"-._( O_ O "._` '. \     
      `"'--._     \  y_     \   \|     
             '-._  \_ _  __.=-.__,\_     
                 `'-(" ,("___       \,_____     
                     (_,("___     .-./     '     
                     |   C'___    (5)     
                     /    ``  '---'-'._```     
                    |     ```    |`    '"-._     
                    |    ````    \-.`     
                    |    ````    |  "._ ``     
                    /    ````    |     '-.__     
                   |     ```     |     
                   |     ```     |     
                   |     ```     |     
                   |     ```     /     
                   |    ````    |     
                   |    ```     |     
                   |    ```     /     
                   |    ```     |     
                   /    ```     |     
                  |     ```     |     
                  |     ```     !     
                  |     ```    / '-.___     
                  |    ````    !_      ''-     
                  /   `   `    | '--._____)     
                  |     /|     !     
                  !    / |     /     
                  |    | |    /     
                  |    | |   /     
                  |    / |   |     
                  /   /  |   |     
                 /   /   |   |     
                (,,_]    (,_,)    mozz   "#
                );
                FreeRtos::delay_ms(1);
                buffer = String::new();
            }
            if next_byte != '\n' {
                buffer.push(next_byte);
                continue;
            }
            process_command(buffer, &tx, &rx);
            buffer = String::new();
        }
        FreeRtos::delay_ms(1);

        if SENDING.load(Ordering::Relaxed) == false {
            // Remove the currently send element
            let Ok(next) = rx.try_recv() else {
                continue;
            };
            SENDING.store(true, Ordering::Relaxed);
            let signal = packet_to_signal(&next);
            TX.lock().unwrap().start(signal).unwrap();
        }
    }
}

fn main() {
    // unsafe {
    //     esp_idf_sys::sleep(2);
    // }
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    unsafe {
        esp!(uart_driver_install(0, 512, 512, 10, null_mut(), 0)).unwrap();
        esp_vfs_dev_uart_use_driver(0);
    }
    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    println!("meow");
    interactive();
}
