use std::sync::Mutex;

use crate::mailbox::Mailbox;

pub struct AppState {
    pub mailbox: Mutex<Mailbox>,
    pub outbox: Mutex<()>,
}
