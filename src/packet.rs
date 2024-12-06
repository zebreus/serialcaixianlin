// signal to the collar:
// [PREFIX        ] = SYNC
// [TRANSMITTER ID] =     XXXXXXXXXXXXXXXX
// [CHANNEL       ] =                     XXXX
// [MODE          ] =                         XXXX
// [STRENGTH      ] =                             XXXXXXXX
// [CHECKSUM      ] =                                     XXXXXXXX
// [END           ] =                                             000

/// Channels one can send a message on
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    Zero,
    One,
    Two,
}

impl From<u8> for Channel {
    /// Convert a u8 to a Channel
    fn from(value: u8) -> Self {
        match value {
            0 => Channel::Zero,
            1 => Channel::One,
            2 => Channel::Two,
            _ => panic!("Invalid channel {}", value),
        }
    }
}

impl Into<Vec<bool>> for Channel {
    /// Convert a Channel to a vector of bits
    fn into(self) -> Vec<bool> {
        match self {
            Channel::Zero => vec![false, false, false, false],
            Channel::One => vec![false, false, false, true],
            Channel::Two => vec![false, false, true, false],
        }
    }
}

/// Actions one can perform
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Action {
    /// Shock the collar
    Shock = 1,
    /// Vibrate the collar
    Vibrate = 2,
    /// Beep the collar
    Beep = 3,
    // Toggle the light on the collar
    Light = 4,
}

impl From<u8> for Action {
    /// Convert a u8 to an Action
    fn from(value: u8) -> Self {
        match value {
            1 => Action::Shock,
            2 => Action::Vibrate,
            3 => Action::Beep,
            4 => Action::Light,
            _ => panic!("Invalid action {}", value),
        }
    }
}

impl Into<Vec<bool>> for Action {
    /// Convert an Action to a vector of bits
    fn into(self) -> Vec<bool> {
        match self {
            Action::Shock => vec![false, false, false, true],
            Action::Vibrate => vec![false, false, true, false],
            Action::Beep => vec![false, false, true, true],
            Action::Light => vec![false, true, false, false],
        }
    }
}

/// Packet to send to the collar
#[derive(Debug, Clone)]
pub struct Packet {
    /// ID of the collar
    pub id: u16,
    /// Channel to send the message on
    pub channel: Channel,
    /// Action to perform
    pub action: Action,
    /// Intensity of the action (0-99)
    pub intensity: u8,
}

impl Packet {
    /// Checksum the data of the packet
    fn checksum(data: &[bool]) -> Vec<bool> {
        let id_1 = data[0..8].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
        let id_2 = data[8..16].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
        let enums = data[16..24].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
        let intensity = data[24..32].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
        let checksum = id_1 as i32 + id_2 as i32 + enums as i32 + intensity as i32;
        let checksum = checksum % 256;
        let checksum: u8 = checksum.try_into().unwrap();

        let mut bits = Vec::new();
        for i in (0..8).rev() {
            bits.push((checksum >> i) & 1 == 1);
        }
        bits
    }
}

impl Into<Vec<bool>> for Packet {
    /// Convert a Packet to a vector of bits
    fn into(self) -> Vec<bool> {
        let mut bits = Vec::new();

        for i in (0..16).rev() {
            // id
            bits.push((self.id >> i) & 1 == 1);
        }

        let channel: Vec<bool> = self.channel.into();
        bits.extend(channel);

        let action: Vec<bool> = self.action.into();
        bits.extend(action);

        for i in (0..8).rev() {
            // intensity
            bits.push((self.intensity >> i) & 1 == 1);
        }

        let checksum = Self::checksum(&bits);
        bits.extend(checksum);

        for _ in 0..3 {
            // zero end bits
            bits.push(false);
        }

        bits
    }
}

#[cfg(test)]
mod tests {
    /// Test encoding and decoding of packets
    #[test]
    fn test_packet() {
        let packet = Packet {
            id: 0b1111010101010000,
            channel: Channel::Zero,
            action: Action::Shock,
            intensity: 0b10101010,
        };
        let bits = packet.to_bits();
        let packet2 = Packet::new(&bits);
        let bits2 = packet2.to_bits();
        assert_eq!(bits, bits2);
    }

    /// Test decoding of packets
    #[test]
    fn decodes_known_packet() {
        let decoded = string_to_vec("00101100 10111110 00000010 00110010 00011110 000");
        let _ = Packet::new(&decoded);
    }

    /// Test reencoding of decoded packets
    #[test]
    fn reencodes_known_packet() {
        let decoded = string_to_vec("00101100 10111110 00000010 00110010 00011110 000");
        let packet = Packet::new(&decoded);
        let reencoded = packet.to_bits();
        assert_eq!(decoded, reencoded);
    }

    /// Test decoding of packets against known values
    #[test]
    fn decodes_known_packet_to_the_expected_value() {
        let decoded = string_to_vec("00101100 10111110 00000010 00110010 00011110 000");
        let packet = Packet {
            id: 0b0010110010111110,
            channel: Channel::Zero,
            action: Action::Vibrate,
            intensity: 0b00110010,
        };
        let generated_packet = packet.to_bits();
        assert_eq!(decoded, generated_packet);
    }
}
