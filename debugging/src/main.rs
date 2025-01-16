use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;

use rspotify::model::Page;
use rspotify::model::SimplifiedEpisode;

fn main() {
    let file = File::open("./lateral-api-response.json").expect("Unable to open file");
    let mut reader = BufReader::new(file);
    let mut contents = String::new();
    reader
        .read_to_string(&mut contents)
        .expect("Unable to read file");

    println!("Loaded{}", contents);

    // Works:
    // match serde_json::from_str::<Page<Option<SimplifiedEpisode>>>(&contents) {
    //     Ok(res) => {
    //         println!("Weird: {:?}", res);
    //     }
    //     Err(err) => {
    //         println!("ERro! {:?}", err);
    //     }
    // }

    // Does not work:
    match serde_json::from_str::<Page<SimplifiedEpisode>>(&contents) {
        Ok(res) => {
            println!("Weird: {:?}", res);
        }
        Err(err) => {
            println!("ERro! {:?}", err);
        }
    }
}
