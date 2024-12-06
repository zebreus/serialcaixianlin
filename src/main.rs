use std::ptr::null_mut;

use esp_idf_hal::delay::FreeRtos;
use esp_idf_sys::{esp, esp_vfs_dev_uart_use_driver, uart_driver_install};

mod cli;
mod packet;
mod queue;

fn main() {
    // setup the peripherals
    esp_idf_svc::sys::link_patches();

    unsafe {
        esp!(uart_driver_install(0, 512, 512, 10, null_mut(), 0)).unwrap();
        esp_vfs_dev_uart_use_driver(0);
    }

    // setup the logger
    esp_idf_svc::log::EspLogger::initialize_default();
    println!("meow :3 arf~");

    // create the transmitter queue and cli state
    let mut queue = unsafe { queue::Queue::new() };
    let mut state = cli::State::new();

    // main loop
    let mut buffer = String::new();
    loop {
        // tick queue
        queue.tick();

        // check for input
        // (note: seems to be the only reliable way to read a single character from stdin afaik)
        let char = unsafe { libc::getchar() };
        if char == -1 {
            FreeRtos::delay_ms(1);
            continue;
        }

        // process input
        let char = char::from(char as u8);
        if char == '\n' {
            cli::process_command(&buffer, &mut state, &queue);
            buffer.clear();
        } else {
            buffer.push(char);
        }

        // check too long command
        if buffer.len() > 100 {
            longcat();
            buffer.clear();
        }

        FreeRtos::delay_ms(1);
    }
}

/// https://mozz.us/ascii-art/2023-05-01/longcat.html
fn longcat() {
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
                (,,_]    (,_,)    mozz"#
    );
}
