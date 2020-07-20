pub mod healthcheck;
pub mod torrent;
pub mod youtube2rss;

use crate::telegram_api::{Message, SendMessage, TelegramClient, Update};
use crate::{Handler, HandlerContext};

use std::error::Error;
use std::sync::mpsc::{channel, sync_channel, Sender, SyncSender};
use std::thread::spawn;
use tokio;


pub fn init_sync_handlers_loop(
    handler_context: HandlerContext<'static>) -> Sender<Update> {
    let (tx, rx) = channel::<Update>();
    spawn(move || {
        let handlers: Vec<Box<Handler>>  = vec![
            Box::new(torrent::TorrentHandler::new(&handler_context)),
            Box::new(healthcheck::HealthCheckHandler::new(&handler_context)),
            Box::new(youtube2rss::PodcastHandler::new(&handler_context)),
        ];
        loop {
            if let Ok(u) = rx.recv() {
                for handler in handlers.iter() {
                if let Err(e) = handler.process(&u.message) {
                    error!(
                        "Problem while processing update {:?} by handler {} with error: {:?}",
                        &u.message,
                        handler.name(),
                        e);
                    send_error_message(&u, &handler.name(), handler_context.telegram_client);
                }
                ack_update(&handler.name(), &u.update_id);
            }
            }
        }
    });
    tx
}


fn store_update(handler_name: &str, update: &Update) {}
fn ack_update(handler_name: &str, update_id: &i32) {}

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