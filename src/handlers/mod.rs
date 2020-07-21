pub mod healthcheck;
pub mod torrent;
pub mod youtube2rss;

use crate::telegram_api::{Message, SendMessage, TelegramClient, Update};
use crate::HANDLER_CONTEXT;

use anyhow::Result;
use async_trait::async_trait;
use lazy_static::lazy_static;
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
    static ref SYNC_HANDLERS: Vec<Box<dyn Handler + Sync + Send>> =
        vec![Box::new(youtube2rss::PodcastHandler::new(&HANDLER_CONTEXT)),];
    static ref ASYNC_HANDLERS: Vec<Box<dyn AsyncHandler + Sync + Send>> = vec![
        Box::new(healthcheck::HealthCheckHandler::new(&HANDLER_CONTEXT)),
        Box::new(torrent::TorrentHandler::new(&HANDLER_CONTEXT)),
    ];
}

pub fn init_sync_handlers_loop() -> Sender<Update> {
    let (tx, rx) = channel::<Update>();
    spawn(move || loop {
        if let Ok(u) = rx.recv() {
            for handler in SYNC_HANDLERS.iter() {
                if let Err(e) = handler.process(&u.message) {
                    error!(
                        "Problem while processing update {:?} by handler {} with error: {:?}",
                        &u.message,
                        handler.name(),
                        e
                    );
                    send_error_message(&u, &handler.name(), HANDLER_CONTEXT.telegram_client);
                }
                ack_update(&handler.name(), &u.update_id);
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
                            if let Err(e) = handler.process(&u.message).await {
                                error!(
                                        "Problem while processing update {:?} by handler {} with error: {:?}",
                                        &u.message,
                                        handler.name(),
                                        e
                                    );
                                async_send_error_message(
                                    &u,
                                    &handler.name()
                                ).await;
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

fn send_error_message(update: &Update, handler_name: &str, telegram_client: &TelegramClient) {
    let message = SendMessage {
        chat_id: update.message.chat.id.to_string(),
        text: format!(
            "что-то пошло не так во время обработки сообщения модулем {}",
            handler_name
        ),
        reply_to_message_id: Some(&update.message.message_id),
    };
    let result = telegram_client.send_message(message);
    match result {
        Err(e) => error!(
            "Problem while trying to send error message for update id {} and handler {} error: {:?}",
            update.update_id, handler_name, e
        ),
        _ => (),
    }
}

async fn async_send_error_message(update: &Update, handler_name: &str) {
    let message = SendMessage {
        chat_id: update.message.chat.id.to_string(),
        text: format!(
            "что-то пошло не так во время обработки сообщения модулем {}",
            handler_name
        ),
        reply_to_message_id: Some(&update.message.message_id),
    };
    let result = crate::TELEGRAM_CLIENT.async_send_message(message).await;
    match result {
        Err(e) => error!(
            "Problem while trying to send error message for update id {} and handler {} error: {:?}",
            update.update_id, handler_name, e
        ),
        _ => (),
    }
}
