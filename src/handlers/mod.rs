pub mod healthcheck;
pub mod torrent;
pub mod youtube2rss;

use crate::telegram_api::{Message, Update};
use crate::{AsyncHandler, Handler};

use std::error::Error;
use std::sync::mpsc::{channel, sync_channel, Sender, SyncSender};
use std::thread::spawn;
use tokio;


pub fn init_sync_handlers_loop(
    make_handlers: fn() -> Vec<Box<dyn Handler>>,
    on_process_error: fn(anyhow::Error) -> ()
) -> Sender<Update> {
    let (tx, rx) = channel::<Update>();
    spawn(move || {
        let handlers: Vec<Box<dyn Handler>> = make_handlers();
        loop {
            if let Ok(u) = rx.recv() {
                for handler in handlers {
                if let Err(e) = handler.process(&u.message) {
                    on_process_error(e);
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