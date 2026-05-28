use std::fs::File;
use rodio::{Decoder, OutputStream};

// Play music function.
// Sorry but they are from rodio's example, currently and I don't know if it works or not. I can't test it out.
// At least, VS Code tells me there 0 errors and 1 warnings (in this function)
// I'm new to Rust. 
fn play_music(file: String) -> OutputStream {
    // Define an stream handle and a sink.
    let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
        .expect("opening default audio stream failed");
    let sink = rodio::Sink::connect_new(&stream_handle.mixer());
    // Load from a file
    let file = File::open(file).unwrap();
    // Decode that sound file into a source
    let source = Decoder::try_from(file).unwrap();
    // Play the sound directly on the device
    stream_handle.mixer().add(source);

    // The sound plays in a separate audio thread,
    // so we need to keep the main thread alive while it's playing.
    std::thread::sleep(std::time::Duration::from_secs(5));

    stream_handle
}

fn main() -> anyhow::Result<()> {
    println!(
        "Hello from local_player member of spotify_player. The player is not implemented yet!"
    );
    println!("Playing sound.mp3 from the current directory...");

    play_music("sound.mp3".to_string());

    Ok(())
}
