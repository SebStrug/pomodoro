use chrono::{Duration as ChronoDuration, Local};
use dirs;
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

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

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

fn start_beeping(beep_control: Arc<Mutex<bool>>, sound_file: &str) {
    // Beep and continue beeping every N seconds until the `beep_control` is set to true
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&stream_handle).unwrap();

    if *beep_control.lock().unwrap() {
        let file = File::open(sound_file).unwrap();
        let source = rodio::Decoder::new(BufReader::new(file)).unwrap();
        sink.append(source);
        thread::sleep(Duration::from_secs(10));
    } else {
        thread::sleep(Duration::from_secs(1));
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
        let timer_duration = ChronoDuration::seconds(10);
        let timer_end = Local::now() + timer_duration;

        // Work timer
        while Local::now() < timer_end {
            thread::sleep(Duration::from_secs(1));
        }
        *beep_control.lock().unwrap() = true;
        println!("⌛ Pomodoro finished, waiting for ack ⌛");
        // Log completed pomodoro to the file
        log_pomodoro();
        start_beeping(beep_control.clone(), "src/beep.mp3");

        // Wait for acknowledgement before starting the break timer
        while *beep_control.lock().unwrap() {
            thread::sleep(Duration::from_secs(1));
        }

        println!("☕ Time for a break ☕");
        let break_duration = ChronoDuration::seconds(5);
        let break_end = Local::now() + break_duration;

        // Break timer
        while Local::now() < break_end {
            thread::sleep(Duration::from_secs(1));
        }
        *beep_control.lock().unwrap() = true;
        println!("⌛ Break finished, waiting for ack ⌛");
        start_beeping(beep_control.clone(), "src/end_break.mp3");

        // Wait for acknowledgement before starting the work timer
        while *beep_control.lock().unwrap() {
            thread::sleep(Duration::from_secs(1));
        }
    }
}

async fn send_ack() {
    // 'Sending an ack' means writing a literal 'ack' to a socket
    let socket_path = "/tmp/pomodoro.sock";
    let mut socket = UnixStream::connect(socket_path).await.unwrap();
    let _ = socket.write_all(b"ack\n").await;
}

async fn process_ack_message(mut socket: tokio::net::UnixStream, beep_control: Arc<Mutex<bool>>) {
    // When 'ack' arg mark `beep_control` as false
    let mut buf = [0u8; 4];
    let _ = socket.read_exact(&mut buf).await;
    let msg = String::from_utf8_lossy(&buf);

    if msg == "ack\n" {
        *beep_control.lock().unwrap() = false;
    }
}

fn log_pomodoro() {
    let path = dirs::home_dir().unwrap().join(".pomodoro-stats");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    writeln!(file, "pomodoro - {}", timestamp).unwrap();
}
