use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};

use crate::{
    packet::{Action, Channel, Packet},
    queue::Queue,
};

/// Print help message
fn print_help() {
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
  light             : Transmit a light toggle command
  transmit [1-1000] : Transmit the configured command the given amount (default 4)
  "#
    );
}

/// State of the interactive shell
pub struct State {
    /// Shocker ID
    pub id: u16,
    /// Channel to transmit on
    pub channel: Channel,
    /// Intensity of the command
    pub intensity: u8,
    /// Action to perform
    pub action: Action,

    /// Last light
    pub last_light_state: bool,

    /// Storage partition
    nvs: EspNvs<NvsDefault>,
}

impl State {
    /// Create a new state
    pub fn new() -> Self {
        // grab storage partition
        let nvs_default_partition = EspDefaultNvsPartition::take().unwrap();
        let nvs = EspNvs::new(nvs_default_partition, "sc_config", true).unwrap();

        // grab values from storage
        let id = nvs.get_u16("id").unwrap_or(Some(0)).unwrap_or(0);
        let intensity = nvs.get_u8("intensity").unwrap_or(Some(1)).unwrap_or(1);
        let action: Action = nvs.get_u8("action").unwrap_or(Some(1)).unwrap_or(1).into();
        let channel: Channel = nvs.get_u8("channel").unwrap_or(Some(0)).unwrap_or(0).into();

        // return state
        Self {
            id,
            channel,
            intensity,
            action,
            last_light_state: false,
            nvs,
        }
    }
    /// Save the state to storage
    fn store(&self) {
        self.nvs.set_u16("id", self.id).unwrap();
        self.nvs.set_u8("intensity", self.intensity).unwrap();
        self.nvs.set_u8("action", self.action as u8).unwrap();
        self.nvs.set_u8("channel", self.channel as u8).unwrap();
    }
}

/// Process a command
pub fn process_command(command: &String, state: &mut State, queue: &Queue) {
    // parse string into <command> <int> format
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
            state.id = id as u16;
            state.store();
            println!("Setting ID to {}", id);
        }

        ("channel" | "c", channel) => {
            if !(0..=2).contains(&channel) {
                println!("Channel must be between 0 and 2");
                return;
            }
            state.channel = Channel::from(channel as u8);
            state.store();
            println!("Setting channel to {}", channel);
        }

        ("intensity" | "i", intensity) => {
            if !(0..=99).contains(&intensity) {
                println!("Intensity must be between 0 and 99");
                return;
            }
            state.intensity = intensity as u8;
            state.store();
            println!("Setting intensity to {}", intensity);
        }

        ("vibrate" | "v", intensity) => {
            if intensity != -1 {
                if !(0..=99).contains(&intensity) {
                    println!("Intensities must be between 0 and 99");
                    return;
                }
                state.intensity = intensity as u8;
            }
            state.action = Action::Vibrate;
            state.store();
            println!("Setting action to vibrate {}", state.intensity);
        }

        ("shock" | "s", intensity) => {
            if intensity != -1 {
                if !(0..=99).contains(&intensity) {
                    println!("Intensities must be between 0 and 99");
                    return;
                }
                state.intensity = intensity as u8;
            }
            state.action = Action::Shock;
            state.store();
            println!("Setting action to shock {}", state.intensity);
        }

        ("beep" | "b", _) => {
            state.action = Action::Beep;
            state.store();
            println!("Setting action to beep");
        }

        ("light" | "l", _) => {
            // build packet
            let intensity = if state.last_light_state { 96 } else { 97 };
            state.last_light_state = !state.last_light_state;

            let packet = Packet {
                id: state.id,
                channel: state.channel,
                action: Action::Light,
                intensity,
            };

            // send packet
            println!(
                "Sending light toggle to shocker {} on channel {:?}",
                state.id, state.channel
            );
            let encoded: Vec<bool> = packet.into();
            for _ in 0..4 {
                queue.send(encoded.clone());
            }
        }

        ("transmit" | "t", amount) => {
            // default amount is 4
            let amount = if amount == -1 { 4 } else { amount };
            if amount < 0 || amount > 65535 {
                println!("Amount must be between 0 and 65535");
                return;
            }

            // build packet
            let packet = Packet {
                id: state.id,
                channel: state.channel,
                action: state.action,
                intensity: state.intensity,
            };

            // send packet
            println!(
                "Sending {:?} to shocker {} on channel {:?} with intensity {}",
                state.action, state.id, state.channel, state.intensity
            );
            let encoded: Vec<bool> = packet.into();
            for _ in 0..amount {
                queue.send(encoded.clone());
            }
        }

        _ => {
            println!("Unknown command {}", command);
            print_help();
        }
    }
}
