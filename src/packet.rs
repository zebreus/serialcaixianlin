#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Channel {
    Zero,
    One,
    Two,
}

impl From<u8> for Channel {
    fn from(value: u8) -> Self {
        match value {
            0 => Channel::Zero,
            1 => Channel::One,
            2 => Channel::Two,
            _ => panic!("Invalid channel {}", value),
        }
    }
}
impl From<u8> for Action {
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
// static (tx, rx): (Sender<i32>, Receiver<i32>) = std::sync::mpsc::channel();

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Action {
    Shock = 1,
    Vibrate = 2,
    Beep = 3,
    // Untested
    Light = 4,
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub id: u16,
    pub channel: Channel,
    pub action: Action,
    pub intensity: u8,
}

fn checksum(data: &[bool]) -> Vec<bool> {
    let id_1 = data[0..8].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    let id_2 = data[8..16].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    let enums = data[16..24].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    let intensity = data[24..32].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    // println!("{:08b}", id_1);
    // println!("{:08b}", id_2);
    // println!("{:08b}", enums);
    // println!("{:08b}", intensity);

    let checksum = id_1 as i32 + id_2 as i32 + enums as i32 + intensity as i32;
    let checksum = checksum % 256;
    let checksum: u8 = checksum.try_into().unwrap();

    let mut bits = Vec::new();
    for i in (0..8).rev() {
        bits.push((checksum >> i) & 1 == 1);
    }
    bits
}

impl Packet {
    pub fn to_bits(&self) -> Vec<bool> {
        let mut bits = Vec::new();
        for i in (0..16).rev() {
            bits.push((self.id >> i) & 1 == 1);
        }
        match self.channel {
            Channel::Zero => bits.extend_from_slice(&[false, false, false, false]),
            Channel::One => bits.extend_from_slice(&[false, false, false, true]),
            Channel::Two => bits.extend_from_slice(&[false, false, true, false]),
        }
        match self.action {
            Action::Shock => bits.extend_from_slice(&[false, false, false, true]),
            Action::Vibrate => bits.extend_from_slice(&[false, false, true, false]),
            Action::Beep => bits.extend_from_slice(&[false, false, true, true]),
            Action::Light => bits.extend_from_slice(&[false, true, false, false]),
        }
        for i in (0..8).rev() {
            bits.push((self.intensity >> i) & 1 == 1);
        }
        let checksum = checksum(&bits);
        bits.extend_from_slice(&checksum);
        for _ in 0..3 {
            bits.push(false);
        }
        bits
    }
}

#[cfg(test)]
mod tests {

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

    #[test]
    fn decodes_known_packet() {
        let decoded = string_to_vec("00101100 10111110 00000010 00110010 00011110 000");
        let _ = Packet::new(&decoded);
    }

    #[test]
    fn reencodes_known_packet() {
        let decoded = string_to_vec("00101100 10111110 00000010 00110010 00011110 000");
        let packet = Packet::new(&decoded);
        let reencoded = packet.to_bits();
        assert_eq!(decoded, reencoded);
    }

    #[test]
    fn decodes_known_packet_to_the_expected_value() {
        let decoded = string_to_vec("00101100 10111110 00000010 00110010 00011110 000");
        // let decoded_packet = Packet::new(&decoded);
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

// [PREFIX        ] = SYNC
// [TRANSMITTER ID] =     XXXXXXXXXXXXXXXX
// [CHANNEL       ] =                     XXXX
// [MODE          ] =                         XXXX
// [STRENGTH      ] =                             XXXXXXXX
// [CHECKSUM      ] =                                     XXXXXXXX
// [END           ] =                                             00

// 00101100
// 10111110
// 00000010
// 00110010
// 00011110

// 00011110 000
