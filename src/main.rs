#[macro_use]
extern crate log;

mod handlers;
mod telegram_api;

use handlers::{init_async_handlers_loop, init_sync_handlers_loop};
use std::env;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use telegram_api::*;

use lazy_static::lazy_static;

use reqwest::{blocking, Client};

pub struct HandlerContext<'a> {
    telegram_client: &'a TelegramClient<'a>,
    http_client: &'a blocking::Client,
    async_http_client: &'a Client,
}

lazy_static! {
    static ref HTTP_CLIENT: blocking::Client = blocking::Client::new();
    static ref ASYNC_HTTP_CLIENT: Client = Client::new();
    static ref TELEGRAM_CLIENT: TelegramClient<'static> = {
        let token =
            env::var("TELEGRAM_TOKEN").expect("Provide TELEGRAM_TOKEN environment variable please");
        telegram_api::TelegramClient::new(token, &HTTP_CLIENT, &ASYNC_HTTP_CLIENT)
    };
    static ref HANDLER_CONTEXT: HandlerContext<'static> = HandlerContext {
        telegram_client: &TELEGRAM_CLIENT,
        http_client: &HTTP_CLIENT,
        async_http_client: &ASYNC_HTTP_CLIENT,
    };

    static ref RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("Error while trying to create tokio runtime");
}

fn main() {
    env_logger::init();

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

    let tx = init_sync_handlers_loop();
    let tx_async = init_async_handlers_loop();

    loop {
        match TELEGRAM_CLIENT.get_updates(update_id + 1) {
            Ok(r) => {
                for update in r.clone().result {
                    tx_async
                        .send(update)
                        .expect("Channel for async handlers is broken");
                }

                for update in r.clone().result {
                    tx.send(update)
                        .expect("Channel for sync handlers is broken");
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
