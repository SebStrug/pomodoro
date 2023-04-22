use chrono::{Duration as ChronoDuration, Local};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::net::UnixStream;

#[tokio::main]
async fn main() {
    println!("Entered program");
    let args: Vec<String> = env::args().collect();

    let beep_control = Arc::new(Mutex::new(false));

    if args.len() == 2 && args[1] == "ack" {
        println!("Received ack!");
        // Send an "ack" message to the server
        send_ack().await;
    } else {
        println!("Received non-ack");
        let timer_handle = {
            let beep_control = beep_control.clone();
            thread::spawn(move || {
                start_pomodoro(beep_control.clone());
            })
        };

        let beeping_handle = {
            let beep_control = beep_control.clone();
            thread::spawn(move || {
                start_beeping(beep_control.clone());
            })
        };

        start_server(beep_control.clone()).await;

        let _ = timer_handle.join();
        let _ = beeping_handle.join();
    }
}

fn start_beeping(beep_control: Arc<Mutex<bool>>) {
    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&stream_handle).unwrap();

    loop {
        if *beep_control.lock().unwrap() {
            let file = File::open("src/beep.mp3").unwrap();
            let source = rodio::Decoder::new(BufReader::new(file)).unwrap();
            sink.append(source);
            thread::sleep(Duration::from_secs(10));
        } else {
            thread::sleep(Duration::from_secs(1));
        }
    }
}

async fn start_server(beep_control: Arc<Mutex<bool>>) {
    let socket_path = "/tmp/pomodoro.sock";
    let _ = std::fs::remove_file(socket_path); // Remove the file if it exists

    let listener = UnixListener::bind(socket_path).unwrap();
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        process_ack_message(socket, beep_control.clone()).await;
    }
}

fn start_pomodoro(beep_control: Arc<Mutex<bool>>) {
    println!("Starting pomodoro");
    loop {
        println!("Started timer");
        let timer_duration = ChronoDuration::seconds(10);
        let timer_end = Local::now() + timer_duration;
        println!("Finished timer");

        // Work timer
        while Local::now() < timer_end {
            thread::sleep(Duration::from_secs(1));
        }
        *beep_control.lock().unwrap() = true;

        // Wait for acknowledgement before starting the break timer
        while *beep_control.lock().unwrap() {
            thread::sleep(Duration::from_secs(1));
        }

        println!("Starting break");
        let break_duration = ChronoDuration::seconds(5);
        let break_end = Local::now() + break_duration;

        // Break timer
        while Local::now() < break_end {
            thread::sleep(Duration::from_secs(1));
        }
        *beep_control.lock().unwrap() = true;

        // Wait for acknowledgement before starting the work timer
        while *beep_control.lock().unwrap() {
            thread::sleep(Duration::from_secs(1));
        }
    }
}

async fn send_ack() {
    let socket_path = "/tmp/pomodoro.sock";
    let mut socket = UnixStream::connect(socket_path).await.unwrap();
    let _ = socket.write_all(b"ack\n").await;
}

async fn process_ack_message(mut socket: tokio::net::UnixStream, beep_control: Arc<Mutex<bool>>) {
    let mut buf = [0u8; 4];
    let _ = socket.read_exact(&mut buf).await;
    let msg = String::from_utf8_lossy(&buf);

    if msg == "ack\n" {
        *beep_control.lock().unwrap() = false; // Stop the beeping
                                               // Start the break timer
    }
}
