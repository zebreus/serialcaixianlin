use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
};

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

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Action {
    Shock = 1,
    Vibrate = 2,
    Beep = 3,
    // Untested
    Light = 4,
}

#[derive(Debug, Clone, Copy)]
pub struct Packet {
    pub id: u16,
    pub channel: Channel,
    pub action: Action,
    pub intensity: u8,
}

fn string_to_vec(s: &str) -> Vec<bool> {
    s.chars()
        .filter(|c| *c == '1' || *c == '0')
        .map(|c| c == '1')
        .collect()
}

fn checksum(data: &[bool]) -> Vec<bool> {
    let id_1 = data[0..8].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    let id_2 = data[8..16].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    let enums = data[16..24].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    let intensity = data[24..32].iter().fold(0, |acc, &x| (acc << 1) + x as u8);
    println!("{:08b}", id_1);
    println!("{:08b}", id_2);
    println!("{:08b}", enums);
    println!("{:08b}", intensity);

    let checksum = id_1 as i32 + id_2 as i32 + enums as i32 + intensity as i32;
    let checksum = checksum % 256;
    let checksum: u8 = checksum.try_into().unwrap();

    let mut bits = Vec::new();
    for i in (0..8).rev() {
        bits.push((checksum >> i) & 1 == 1);
    }
    return bits;
}

impl Packet {
    pub fn new(data: &[bool]) -> Self {
        // The id is the first 16 bits as a u16
        let id: u16 = data[0..16].iter().fold(0, |acc, &x| (acc << 1) + x as u16);
        let channel = match (data[16], data[17], data[18], data[19]) {
            (false, false, false, false) => Channel::Zero,
            (false, false, false, true) => Channel::One,
            (false, false, true, false) => Channel::Two,
            any => panic!("Invalid channel {:?}", any),
        };
        let action = match (data[20], data[21], data[22], data[23]) {
            (false, false, false, true) => Action::Shock,
            (false, false, true, false) => Action::Vibrate,
            (false, false, true, true) => Action::Beep,
            any => panic!("Invalid action {:?}", any),
        };
        let intensity: u8 = data[24..32].iter().fold(0, |acc, &x| (acc << 1) + x as u8);

        let checksum = checksum(data);
        assert_eq!(&checksum, &data[32..40]);
        Packet {
            id,
            channel,
            action,
            intensity,
        }
    }

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
        // for bit in &bits {
        //     print!("{}", if *bit { "1" } else { "0" });
        // }
        // println!();
        return bits;
    }

    pub fn write(&self, writer: &mut BufWriter<File>) {
        let bitstream = self.to_bits();
        let samplerate = 20_000_000;
        let micros_sync = 1400;
        let micros_short = 250;
        let micros_long = 750;

        let samples_sync = (micros_sync as f32 * (samplerate as f32 / 1_000_000.0)) as usize;
        let samples_short = (micros_short as f32 * (samplerate as f32 / 1_000_000.0)) as usize;
        let samples_long = (micros_long as f32 * (samplerate as f32 / 1_000_000.0)) as usize;

        for _ in 0..samples_sync {
            writer.write_all(&[1]).unwrap();
        }
        for _ in 0..samples_long {
            writer.write_all(&[0]).unwrap();
        }

        for bit in &bitstream {
            if *bit {
                for _ in 0..samples_long {
                    writer.write_all(&[1]).unwrap();
                }
                for _ in 0..samples_short {
                    writer.write_all(&[0]).unwrap();
                }
            } else {
                for _ in 0..samples_short {
                    writer.write_all(&[1]).unwrap();
                }
                for _ in 0..samples_long {
                    writer.write_all(&[0]).unwrap();
                }
            }
            // buf.write_all(&[if bit { 1 } else { 0 }]).unwrap();
        }
    }
}

// fn main() {
//     println!("Hello, world!");
//     let mut file = File::open("/home/lennart/Documents/decoded.rec").unwrap();
//     let mut buf = BufReader::new(file);
//     let bytes = buf.bytes();

//     let mut run_length: u32 = 0;
//     let mut low_length: u32 = 0;
//     let mut buffer: Vec<bool> = Vec::new();

//     for byte in bytes {
//         let byte = byte.unwrap();
//         let bit: bool = byte == 1;
//         if bit {
//             run_length += 1;
//             low_length = 0;
//         } else {
//             if run_length >= 6 && run_length <= 12 {
//                 // print!("Low");
//                 buffer.push(false);
//             }
//             if run_length >= 20 && run_length <= 32 {
//                 buffer.push(true);
//             }
//             if run_length >= 38 && run_length <= 52 {
//                 println!("Sync");
//                 print!("Message: ");
//                 for b in &buffer {
//                     print!("{}", if *b { "1" } else { "0" });
//                 }
//                 println!("");

//                 buffer.clear();
//             }

//             low_length += 1;
//             run_length = 0;
//         }
//     }
// }

fn main() {
    println!("Hello, world!");
    let file = File::create("/home/lennart/Documents/output.rec").unwrap();
    let mut buf = BufWriter::new(file);

    // for x in 0..33 {
    //     let packet = Packet {
    //         id: 0b0010110010111110,
    //         channel: Channel::Zero,
    //         action: Action::Shock,
    //         intensity: 1 + x * 3,
    //     };
    //     for _ in 0..50 {
    //         packet.write(&mut buf);
    //     }
    //     let packet = Packet {
    //         id: 0b0010110010111110,
    //         channel: Channel::Zero,
    //         action: Action::Beep,
    //         intensity: 1 + x * 3,
    //     };
    //     for _ in 0..5 {
    //         packet.write(&mut buf);
    //     }
    // }

    // for x in 0..25 {
    //     let packet = Packet {
    //         id: 0b0010110010111110,
    //         channel: Channel::Zero,
    //         action: Action::Shock,
    //         intensity: x,
    //     };
    //     for _ in 0..3 {
    //         packet.write(&mut buf);
    //     }
    // }

    // let low_packet = Packet {
    //     id: 0b0010110010111110,
    //     channel: Channel::Zero,
    //     action: Action::Shock,
    //     intensity: 1,
    // };
    // let high_packet = Packet {
    //     id: 0b0010110010111110,
    //     channel: Channel::Zero,
    //     action: Action::Shock,
    //     intensity: 25,
    // };
    // for _ in 0..25 {
    //     low_packet.write(&mut buf);
    // }
    // for _ in 0..1 {
    //     high_packet.write(&mut buf);
    // }
    // for _ in 0..25 {
    //     low_packet.write(&mut buf);
    // }

    buf.flush().unwrap();

    // for byte in bytes {
    //     let byte = byte.unwrap();
    //     let bit: bool = byte == 1;
    //     if bit {
    //         run_length += 1;
    //         low_length = 0;
    //     } else {
    //         if run_length >= 6 && run_length <= 12 {
    //             // print!("Low");
    //             buffer.push(false);
    //         }
    //         if run_length >= 20 && run_length <= 32 {
    //             buffer.push(true);
    //         }
    //         if run_length >= 38 && run_length <= 52 {
    //             println!("Sync");
    //             print!("Message: ");
    //             for b in &buffer {
    //                 print!("{}", if *b { "1" } else { "0" });
    //             }
    //             println!("");

    //             buffer.clear();
    //         }

    //         low_length += 1;
    //         run_length = 0;
    //     }
    // }
}

/// Test that the packet is correctly decoded and encoded
#[cfg(test)]
mod tests {
    use super::*;

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
