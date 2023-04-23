use chrono::{Duration as ChronoDuration, Local};
use dirs;
use rodio::Source;
use std::env;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufReader;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::net::UnixStream;

const BEEP: &[u8] = include_bytes!("../resources/beep.mp3");
const END_BREAK: &[u8] = include_bytes!("../resources/end_break.mp3");

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h" || args[1] == "help") {
        print_help();
        return;
    }

    let beep_control = Arc::new(Mutex::new(false));

    if args.len() == 2 && args[1] == "ack" {
        // Send an "ack" message to the server
        send_ack().await;
    } else {
        let timer_handle = {
            let beep_control = beep_control.clone();
            thread::spawn(move || {
                start_pomodoro(beep_control.clone());
            })
        };

        start_server(beep_control.clone()).await;

        let _ = timer_handle.join();
    }
}

async fn start_server(beep_control: Arc<Mutex<bool>>) {
    // Create a local/UNIX domain socket. Will listen in on socket for an ack.
    let socket_path = "/tmp/pomodoro.sock";
    let _ = std::fs::remove_file(socket_path); // Remove the file if it exists

    let listener = UnixListener::bind(socket_path).unwrap();
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        process_ack_message(socket, beep_control.clone()).await;
    }
}

fn start_pomodoro(beep_control: Arc<Mutex<bool>>) {
    loop {
        println!("🍅 Starting pomodoro! 🍅");
        let timer_duration = ChronoDuration::minutes(25);
        let timer_end = Local::now() + timer_duration;

        // Work timer
        while Local::now() < timer_end {
            thread::sleep(Duration::from_secs(1));
        }
        println!("⌛ Pomodoro finished, waiting for ack ⌛");
        *beep_control.lock().unwrap() = true;
        log_pomodoro();
        start_beeping(beep_control.clone(), BEEP);
        while *beep_control.lock().unwrap() {
            // Waiting indefinitely for acknowledgment
        }

        // Wait for acknowledgement before starting the break timer
        while *beep_control.lock().unwrap() {
            thread::sleep(Duration::from_secs(1));
        }

        println!("☕ Time for a break ☕");
        let break_duration = ChronoDuration::minutes(10);
        let break_end = Local::now() + break_duration;

        // Break timer
        while Local::now() < break_end {
            thread::sleep(Duration::from_secs(1));
        }
        println!("⌛ Break finished, waiting for ack ⌛");
        *beep_control.lock().unwrap() = true;
        start_beeping(beep_control.clone(), END_BREAK);

        // Wait for acknowledgement before starting the work timer
        while *beep_control.lock().unwrap() {
            thread::sleep(Duration::from_secs(1));
        }
        print!("");
    }
}

fn start_beeping(beep_control: Arc<Mutex<bool>>, sound_data: &[u8]) {
    // Beep and continue beeping every N seconds until the `beep_control` is set to true
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&stream_handle).unwrap();

    let tmp_dir = std::env::temp_dir();
    let tmp_file_path = tmp_dir.join("tmp_sound.mp3");
    let mut tmp_file = File::create(&tmp_file_path).expect("Failed to create temporary file");
    tmp_file
        .write_all(sound_data)
        .expect("Failed to write sound data to temporary file");

    // Load the sound file using the temporary file path
    let file = File::open(&tmp_file_path).unwrap();
    let source = rodio::Decoder::new(BufReader::new(file)).unwrap();

    // Set the sound to repeat and play it
    sink.append(source.repeat_infinite());
    sink.play();

    // Wait until the `beep_control` is set to true
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if *beep_control.lock().unwrap() {
            break;
        }
    }

    // Stop the sound and clean up the temporary file
    sink.stop();
    std::fs::remove_file(tmp_file_path).expect("Failed to remove temporary file");
}

async fn send_ack() {
    let socket_path = "/tmp/pomodoro.sock";
    match UnixStream::connect(socket_path).await {
        Ok(mut socket) => {
            let _ = socket.write_all(b"ack\n").await;
        }
        Err(e) => {
            eprintln!("Error connecting to the socket: {}", e);
            eprintln!("Make sure the Pomodoro timer is running.");
        }
    }
}

async fn process_ack_message(mut socket: tokio::net::UnixStream, beep_control: Arc<Mutex<bool>>) {
    // When 'ack' arg mark `beep_control` as false
    let mut buf = [0u8; 4];
    let _ = socket.read_exact(&mut buf).await;
    let msg = String::from_utf8_lossy(&buf);

    if msg == "ack\n" {
        println!("Received ack!");
        *beep_control.lock().unwrap() = false;
    }
}

fn log_pomodoro() {
    // Log pomodoro to file `~/.pomodoro-stats`
    let path = dirs::home_dir().unwrap().join(".pomodoro-stats");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "pomodoro - {}", timestamp).unwrap();
}

fn print_help() {
    println!(
        r#"Pomodoro Timer CLI

Usage:
    pomodoro                      Start the pomodoro timer
    pomodoro ack                  Acknowledge the timer and start the break
    pomodoro --help | -h | help   Display this help message

Description:
    * A simple command-line pomodoro timer. Starts a 25-minute pomodoro timer,
      followed by a 5-minute break. The timer will beep at the end of each phase.
    * Acknowledge the timer with 'pomodoro ack' in another window to start the break or the next pomodoro.
    * Pomodoros are logged to '~/.pomodoro-stats'"#
    );
}
