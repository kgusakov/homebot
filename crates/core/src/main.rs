use handler_core::AsyncHandler;
use handler_core::Handler;
use handler_core::HandlerContext;
use log::error;

use downloader::DownloaderHandler;
use healthcheck;
use telegram_api::Message;
use telegram_api::SendMessage;
use torrent;
use youtube2rss;

use std::sync::mpsc::{Sender, channel};
use std::thread::spawn;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use reqwest::Url;
use std::collections::HashSet;
use std::env;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::iter::FromIterator;
use telegram_api::TelegramClient;
use telegram_api::Update;

use lazy_static::lazy_static;

use reqwest::{Client, blocking};

lazy_static! {
    static ref HTTP_CLIENT: blocking::Client = {
        let proxy = Url::parse(
            &env::var("SOCKS_PROXY").expect("Provide SOCKS_PROXY environment variable please"),
        )
        .expect("Can't parse SOCKS_PROXY url");
        blocking::Client::builder()
            .proxy(reqwest::Proxy::all(proxy).expect("Can't initialize socks proxy"))
            .build()
            .expect("Error during initializing of http client with socks proxy")
    };
    static ref ASYNC_HTTP_CLIENT: Client = Client::new();
    static ref ASYNC_PROXY_HTTP_CLIENT: Client = {
        let proxy = Url::parse(
            &env::var("SOCKS_PROXY").expect("Provide SOCKS_PROXY environment variable please"),
        )
        .expect("Can't parse SOCKS_PROXY url");
        Client::builder()
            .proxy(reqwest::Proxy::all(proxy).expect("Can't initialize socks proxy"))
            .build()
            .expect("Error during initializing of http client with socks proxy")
    };
    static ref TELEGRAM_CLIENT: TelegramClient<'static> = {
        let token =
            env::var("TELEGRAM_TOKEN").expect("Provide TELEGRAM_TOKEN environment variable please");
        telegram_api::TelegramClient::new(token, &HTTP_CLIENT, &ASYNC_PROXY_HTTP_CLIENT)
    };
    static ref HANDLER_CONTEXT: HandlerContext<'static> = HandlerContext {
        telegram_client: &TELEGRAM_CLIENT,
        async_http_client: &ASYNC_HTTP_CLIENT,
        async_proxy_http_client: &ASYNC_PROXY_HTTP_CLIENT,
    };
    static ref RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("Error while trying to create tokio runtime");
    static ref SYNC_HANDLERS: Vec<Box<dyn Handler + Sync + Send>> = vec![];
    static ref ASYNC_HANDLERS: Vec<Box<dyn AsyncHandler + Sync + Send>> = {
        let mut handlers: Vec<Box<dyn AsyncHandler + Sync + Send>> = vec![];
        #[cfg(feature = "healthcheck")]
        handlers.push(Box::new(healthcheck::HealthCheckHandler::new(
            &HANDLER_CONTEXT,
        )));

        #[cfg(feature = "torrent")]
        handlers.push(Box::new(torrent::TorrentHandler::new(&HANDLER_CONTEXT)));

        #[cfg(feature = "youtube2rss")]
        handlers.push(Box::new(youtube2rss::PodcastHandler::new(&HANDLER_CONTEXT)));

        #[cfg(feature = "downloader")]
        handlers.push(Box::new(DownloaderHandler::new(&HANDLER_CONTEXT)));

        handlers
    };
}

fn main() {
    env_logger::init();

    let state_file_path =
        env::var("BOT_STATE_PATH").expect("Provide BOT_STATE_PATH environment variable please");

    let white_list: HashSet<i64> = HashSet::from_iter(
        env::var("USERS_WHITE_LIST")
            .expect("Provide USERS_WHITE_LIST environment variable please")
            .split(",")
            .map(|s| {
                s.parse::<i64>()
                    .expect("Environment variable USERS_WHITE_LIST has the wrong chat ids")
            }),
    );

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
                    match &update {
                        Update {
                            update_id: _,
                            message: Some(m),
                        } if white_list.contains(&m.chat.id) => tx_async
                            .send(update)
                            .expect("channel for async handlers is broken"),
                        _ => (),
                    }
                }

                for update in r.clone().result {
                    match &update {
                        Update {
                            update_id: _,
                            message: Some(m),
                        } if white_list.contains(&m.chat.id) => tx
                            .send(update)
                            .expect("Channel for async handlers is broken"),
                        _ => (),
                    }
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

pub fn init_sync_handlers_loop() -> Sender<Update> {
    let (tx, rx) = channel::<Update>();
    spawn(move || {
        loop {
            match rx.recv() {
                Ok(
                    ref upd @ Update {
                        update_id: u_id,
                        message: ref m,
                    },
                ) => {
                    for handler in SYNC_HANDLERS.iter() {
                        if let Some(message) = m {
                            if let Err(e) = handler.process(&message) {
                                error!(
                                    "Problem while processing update {:?} by handler {} with error: {:?}",
                                    &message,
                                    handler.name(),
                                    e
                                );
                                send_error_message(
                                    &upd.update_id,
                                    message,
                                    &handler.name(),
                                    HANDLER_CONTEXT.telegram_client,
                                );
                            }
                        }
                        ack_update(&handler.name(), &u_id);
                    }
                }
                _ => panic!("Unexpected end of stream for async processing"),
            }
        }
    });
    tx
}

pub fn init_async_handlers_loop() -> UnboundedSender<Update> {
    let (tx, mut rx) = unbounded_channel::<Update>();

    let f = async move {
        loop {
            match rx.recv().await {
                Some(update) => {
                    for handler in ASYNC_HANDLERS.iter() {
                        let u = update.clone();
                        crate::RUNTIME.spawn(async move {
                            if let Some(m) = &u.message {
                                    if let Err(e) = handler.process(m).await {
                                        error!(
                                                "Problem while processing update {:?} by handler {} with error: {:?}",
                                                &u.message,
                                                handler.name(),
                                                e
                                            );
                                        async_send_error_message(
                                            &u.update_id,
                                            m,
                                            &handler.name()
                                        ).await;
                                    }

                            }

                            ack_update(&handler.name(), &u.update_id);
                        });
                    }
                }
                None => panic!("Unexpected end of stream for async processing"),
            }
        }
    };
    crate::RUNTIME.spawn(f);
    tx
}

fn ack_update(_handler_name: &str, _update_id: &i32) {}

fn send_error_message(
    update_id: &i32,
    message: &Message,
    handler_name: &str,
    telegram_client: &TelegramClient,
) {
    let message = SendMessage {
        chat_id: message.chat.id.to_string(),
        text: format!(
            "что-то пошло не так во время обработки сообщения модулем {}",
            handler_name
        ),
        reply_to_message_id: Some(&message.message_id),
    };
    let result = telegram_client.send_message(message);
    match result {
        Err(e) => error!(
            "Problem while trying to send error message for update id {} and handler {} error: {:?}",
            update_id, handler_name, e
        ),
        _ => (),
    }
}

async fn async_send_error_message(update_id: &i32, message: &Message, handler_name: &str) {
    let message = SendMessage {
        chat_id: message.chat.id.to_string(),
        text: format!(
            "что-то пошло не так во время обработки сообщения модулем {}",
            handler_name
        ),
        reply_to_message_id: Some(&message.message_id),
    };
    let result = crate::TELEGRAM_CLIENT.async_send_message(message).await;
    match result {
        Err(e) => error!(
            "Problem while trying to send error message for update id {} and handler {} error: {:?}",
            update_id, handler_name, e
        ),
        _ => (),
    }
}
