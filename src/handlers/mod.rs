#[cfg(feature = "downloader")]
mod downloader;
#[cfg(feature = "healthcheck")]
mod healthcheck;
#[cfg(feature = "torrent")]
mod torrent;
#[cfg(feature = "youtube2rss")]
mod youtube2rss;

use crate::handlers::downloader::DownloaderHandler;
use crate::telegram_api::{Message, SendMessage, TelegramClient, Update};
use crate::HANDLER_CONTEXT;

use anyhow::Result;
use async_trait::async_trait;
use lazy_static::lazy_static;
use log::error;
use std::sync::mpsc::{channel, Sender};
use std::thread::spawn;
use tokio;

use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

trait Handler {
    fn name(&self) -> String;

    fn process(&self, m: &Message) -> Result<()>;
}

#[async_trait]
trait AsyncHandler {
    fn name(&self) -> String;

    async fn process(&self, m: &Message) -> Result<()>;
}

lazy_static! {
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

pub fn init_sync_handlers_loop() -> Sender<Update> {
    let (tx, rx) = channel::<Update>();
    spawn(move || loop {
        match rx.recv() {
            Ok(
                ref upd @ Update {
                    update_id: u_id,
                    message: ref m,
                },
            ) => {
                for handler in SYNC_HANDLERS.iter() {
                    if let Some(ref message) = m {
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
