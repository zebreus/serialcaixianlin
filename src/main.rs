use esp_idf_hal::delay::FreeRtos;

mod cli;
mod packet;
mod queue;

fn main() {
    println!("meow :3 arf~");

    // create the transmitter queue and cli state
    let mut queue = unsafe { queue::init() };
    let mut state = cli::State::new();

    // main loop
    let mut buffer = String::new();
    loop {
        // tick queue
        queue.tick();

        // check for input
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
    (,,_]    (,_,)    mozz   "#
    );
}
