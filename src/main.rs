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
use std::{
    collections::VecDeque,
    sync::{Mutex, RwLock},
};
use std::{sync::LazyLock, time::Duration};
use {
    esp_idf_sys::{esp, esp_vfs_dev_uart_use_driver, uart_driver_install},
    std::ptr::null_mut,
};
mod packet;

struct Pulses {
    pub sync_high: Pulse,
    pub sync_low: Pulse,
    pub one_high: Pulse,
    pub one_low: Pulse,
    pub zero_high: Pulse,
    pub zero_low: Pulse,
}

static PERIPHERALS: LazyLock<Mutex<Peripherals>> =
    LazyLock::new(|| Mutex::new(Peripherals::take().unwrap()));

static PULSES: LazyLock<Pulses> = LazyLock::new(|| {
    let ticks_hz = TX_MUTEX.lock().unwrap().counter_clock().unwrap();

    let sync_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(1450)).unwrap();
    let sync_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(800)).unwrap();

    let one_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(800)).unwrap();
    let one_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(300)).unwrap();

    let zero_high =
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_micros(350)).unwrap();
    let zero_low =
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_micros(750)).unwrap();

    Pulses {
        sync_high,
        sync_low,
        one_high,
        one_low,
        zero_high,
        zero_low,
    }
});
static TX_MUTEX: LazyLock<Mutex<TxRmtDriver<'static>>> = LazyLock::new(|| {
    let mut peripherals = PERIPHERALS.lock().unwrap();

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

// TODO: Figure out a way to do this without global muts and race conditions.
//       The first attempt resulted in deadlocks; the current attempt has ub
static mut QUEUE: VecDeque<Packet> = VecDeque::new();
// Super-unsafe way to access a global mutable ref to tx
static mut TX: LazyLock<&mut TxRmtDriver<'static>> = LazyLock::new(|| {
    let mut tx = TX_MUTEX.lock().unwrap();
    let locked_mutex: &mut TxRmtDriver<'static> = &mut tx;
    let ptr = unsafe {
        std::mem::transmute::<_, *mut TxRmtDriver<'static>>(
            locked_mutex as *mut TxRmtDriver<'static>,
        )
    };
    let ptr = unsafe { std::mem::transmute::<_, *mut std::ffi::c_void>(ptr) };
    let ptr = unsafe { std::mem::transmute::<_, *mut TxRmtDriver<'static>>(ptr) };
    let unptr = unsafe { &mut *ptr };

    unsafe {
        rmt_register_tx_end_callback(Some(tx_complete_callback), null_mut());
    }

    unptr
});
fn add_to_queue(packet: Packet) {
    unsafe {
        let send_now = QUEUE.len() == 0;
        if send_now {
            let signal = packet_to_signal(&packet);
            QUEUE.push_back(packet);
            let tx = LazyLock::get_mut(&mut TX).unwrap();
            tx.start(signal).unwrap();
        } else {
            QUEUE.push_back(packet);
        }
    }
}
extern "C" fn tx_complete_callback(_channel: u32, _arg: *mut std::ffi::c_void) {
    // let mut queue = QUEUE.write().unwrap();

    unsafe {
        // Remove the currently send element
        QUEUE.pop_front();
        let Some(next) = QUEUE.front() else {
            return;
        };
        let tx = LazyLock::get_mut(&mut TX).unwrap();
        tx.start(packet_to_signal(next)).unwrap();
    }
}

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

struct Config {
    pub id: u16,
    pub channel: Channel,
    pub intensity: u8,
    pub action: Action,
    pub nvs: EspNvs<NvsDefault>,
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

fn print_help() {
    println!(
        r#"Available commands:
    help             : Print this help page
    id 0-65535       : Set the id of this transmitter
    channel 0-2      : Set the channel of this transmitter
    shock [0-100]    : Set the command type to zapping
    vibrate [0-100]  : Set the command type to good vibrations
    beep             : Set the command type to make beepy noises
    light            : Set the command type to enable the light
    transmit [0-255] : Transmit the configured command the given amount
    abort            : Stop any ongoing transmission
    "#
    );
}
fn process_command(command: String) {
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
            if !(0..=100).contains(&intensity) {
                println!("Intensity must be 100 or lower");
                return;
            }
            let mut config = CONFIG.write().unwrap();
            config.intensity = intensity as u8;
            config.nvs.set_u8("intensity", intensity as u8).unwrap();
            println!("Setting intensity to {}", intensity);
        }
        ("vibrate" | "v", intensity) => {
            let mut config = CONFIG.write().unwrap();
            if intensity != -1 {
                if !(0..=100).contains(&intensity) {
                    println!("Intensity must be 100 or lower");
                    return;
                }
                config.intensity = intensity as u8;
                config.nvs.set_u8("intensity", intensity as u8).unwrap();
            }
            config.action = Action::Vibrate;
            config.nvs.set_u8("action", Action::Vibrate as u8).unwrap();
            println!("Setting action to vibrate {}", intensity);
        }
        ("shock" | "s", intensity) => {
            let mut config = CONFIG.write().unwrap();
            if intensity != -1 {
                if !(0..=100).contains(&intensity) {
                    println!("Intensity must be 100 or lower");
                    return;
                }
                config.intensity = intensity as u8;
                config.nvs.set_u8("intensity", intensity as u8).unwrap();
            }
            config.action = Action::Shock;
            config.nvs.set_u8("action", Action::Shock as u8).unwrap();
            println!("Setting action to shock {}", intensity);
        }
        ("beep" | "b", _) => {
            let mut config = CONFIG.write().unwrap();
            config.action = Action::Beep;
            config.nvs.set_u8("action", Action::Beep as u8).unwrap();
            println!("Setting action to beep");
        }
        ("light" | "l", _) => {
            let mut config = CONFIG.write().unwrap();
            config.action = Action::Light;
            config.nvs.set_u8("action", Action::Light as u8).unwrap();
            println!("Setting action to light");
        }
        ("transmit" | "t", amount) => {
            let config = CONFIG.read().unwrap();

            let packet = Packet {
                id: config.id,
                channel: config.channel,
                action: config.action,
                intensity: config.intensity,
            };

            println!("Transmitting packet {:?}", packet);

            for _ in 0..amount {
                add_to_queue(packet.clone());
            }
        }
        _ => {
            println!("Unknown command {}", command);
            print_help();
        }
    }
}

fn interactive() {
    let mut buffer = String::new();
    loop {
        unsafe {
            let next = libc::getchar();
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
                FreeRtos::delay_ms(1);
                process_command(buffer);
                buffer = String::new();
            } else {
                FreeRtos::delay_ms(1);
            }
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
