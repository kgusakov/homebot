mod telegram_api;
mod torrent;

use telegram_api::*;
use std::env;

use lazy_static::lazy_static;

pub type ProcessingResult = Result<(), Box<dyn std::error::Error>>;

lazy_static! {
    static ref TELEGRAM_CLIENT: TelegramClient = {
        let token = env::var("TELEGRAM_TOKEN")
            .expect("Provide TELEGRAM_TOKEN environment variable please");
        telegram_api::TelegramClient::new(token)
    };

    static ref FILTERS: [fn(&Message) -> ProcessingResult; 1] = [
        |m| torrent::TorrentFilter::new(&TELEGRAM_CLIENT).process(m)
    ];
}

fn main() -> ProcessingResult {
    let resp = TELEGRAM_CLIENT.get_updates()?;
    process_updates(resp.result, &FILTERS)
}

fn process_updates(updates: Vec<Update>, handlers: &FILTERS) -> ProcessingResult {
    for update in updates.iter() {
        for handler in handlers.iter() {
            handler(&update.message)?;
        }
    }
    Ok(())
}