#[macro_use]
extern crate log;

mod handlers;
mod telegram_api;

use handlers::healthcheck;
use handlers::torrent;
use handlers::youtube2rss;
use handlers::init_sync_handlers_loop;

use anyhow::Result;

use std::env;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use telegram_api::*;

use lazy_static::lazy_static;

use reqwest::blocking::Client;

pub struct HandlerContext<'a> {
    telegram_client: &'a TelegramClient<'a>,
    http_client: &'a Client,
}

trait Handler {
    fn name(&self) -> String;

    fn process(&self, m: &Message) -> Result<()>;
}

lazy_static! {
    static ref HTTP_CLIENT: Client = Client::new();
    
    static ref TELEGRAM_CLIENT: TelegramClient<'static> =  {
        let token =
            env::var("TELEGRAM_TOKEN").expect("Provide TELEGRAM_TOKEN environment variable please");
        telegram_api::TelegramClient::new(token, &HTTP_CLIENT)
    };

}

fn main() {
    env_logger::init();

    let handler_context = HandlerContext {
        telegram_client: &TELEGRAM_CLIENT,
        http_client: &HTTP_CLIENT,
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

    let tx = init_sync_handlers_loop(handler_context);

    loop {
        match TELEGRAM_CLIENT.get_updates(update_id + 1) {
            Ok(r) => {
                for update in r.clone().result {
                    tx.send(update).expect("Channel for sync handlers is broken");
                }
                update_id = r
                    .result
                    .iter()
                    .map(|u| u.update_id)
                    .max()
                    .unwrap_or(update_id);
                save_update_id(&state_file_path, update_id);
            }
            Err(e) => error!(
                "Error while getting updates with offset {} error: {:?}",
                update_id, e
            ),
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
