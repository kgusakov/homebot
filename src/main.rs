#[macro_use]
extern crate log;

mod handlers;
mod telegram_api;

use handlers::healthcheck;
use handlers::torrent;
use handlers::youtube2rss;

use anyhow::Result;
use std::env;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use telegram_api::*;

trait Filter {
    fn process(&self, m: &Message) -> Result<()>;
}

fn main() {
    env_logger::init();

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
        file.read_to_string(&mut contents)
            .expect("Error while trying to read state file content");
        contents
            .trim()
            .parse::<i32>()
            .expect("Bot state is corrupted")
    };

    let filters: Vec<Box<dyn Filter>> = vec![
        Box::new(torrent::TorrentFilter::new(&telegram_client)),
        Box::new(healthcheck::HealthCheckFilter::new(&telegram_client)),
        Box::new(youtube2rss::PodcastFilter::new(&telegram_client)),
    ];

    loop {
        match telegram_client.get_updates(update_id + 1) {
            Ok(r) => {
                process_updates(&r.result, &filters);
                update_id = r
                    .result
                    .iter()
                    .map(|u| u.update_id)
                    .max()
                    .unwrap_or(update_id);
                save_update_id(&state_file_path, update_id);
            }
            Err(e) => error!(
                "Error while getting updates with offset {} error: {}",
                update_id, e
            ),
        }
    }
}

fn process_updates<'a, T: ?Sized>(updates: &Vec<Update>, handlers: &Vec<Box<T>>)
where
    T: Filter,
{
    for update in updates.iter() {
        for handler in handlers.iter() {
            match handler.process(&update.message) {
                Ok(_) => (),
                Err(e) => error!(
                    "Problem while processing update {:?} error: {}",
                    &update.message, e
                ),
            }
        }
    }
}

fn save_update_id(state_path: &str, update_id: i32) {
    match OpenOptions::new().write(true).open(state_path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(update_id.to_string().as_bytes()) {
                error!(
                    "Can't save new state {} to bot state file: {}",
                    update_id, e
                );
            }
        }
        Err(e) => error!(
            "Can't open file with bot state for writing new state {}: {}",
            update_id, e
        ),
    }
}
