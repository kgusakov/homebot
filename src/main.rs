mod telegram_api;
mod torrent;

use std::env;
use std::fs::OpenOptions;
use telegram_api::*;
use std::io::Read;
use std::io::Write;

pub type ProcessingResult = Result<(), Box<dyn std::error::Error>>;

trait Filter {
    fn process(&self, m: &Message) -> ProcessingResult;
}

fn main() -> ProcessingResult {
    let telegram_client: TelegramClient = {
        let token =
            env::var("TELEGRAM_TOKEN").expect("Provide TELEGRAM_TOKEN environment variable please");
        telegram_api::TelegramClient::new(token)
    };

    let state_file_path =
        env::var("BOT_STATE_PATH").expect("Provide BOT_STATE_PATH environment variable please");
    

    let mut update_id = {
        let mut file = OpenOptions::new()
            .read(true)
            .open(&state_file_path)
            .expect("Can't open file with bot state");

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        contents.trim().parse::<i32>().expect("Bot state is corrupted")
    };

    // let filters = filters(&telegram_client);
    let filters = vec![Box::new(torrent::TorrentFilter::new(&telegram_client))];

    loop {
        let resp = telegram_client.get_updates(update_id + 1)?;
        process_updates(&resp.result, &filters)?;
        update_id = resp.result.iter().map(|u| u.update_id).max().unwrap_or(update_id);
        save_update_id(&state_file_path, update_id);
    }
}

fn process_updates<'a, T: ?Sized>(updates: &Vec<Update>, handlers: &Vec<Box<T>>) -> ProcessingResult where T: Filter {
    for update in updates.iter() {
        for handler in handlers.iter() {
            handler.process(&update.message)?;
        }
    }
    Ok(())
}

fn save_update_id(state_path: &str, update_id: i32) {
    let mut f = OpenOptions::new()
            .write(true)
            .open(state_path)
            .expect("Can't open file with bot state for writing new state");
    f.write_all(update_id.to_string().as_bytes()).expect("Can't save new state to bot state file");
}
