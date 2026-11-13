// SPDX-License-Identifier: GPL-3.0-or-later

//! How work reaches the window: messages from the views and the player,
//! and the results of work done off the main thread, all through one
//! channel the window reads in order on the main loop.

use std::future::Future;
use std::sync::OnceLock;

use super::{AppCommand, AppMsg};

/// What the window handles next.
pub enum Event {
    Message(AppMsg),
    Command(AppCommand),
}

/// Sends to the window from anywhere, any thread included.
#[derive(Clone)]
pub struct AppSender(async_channel::Sender<Event>);

/// The runtime the library's network and file work runs on: reqwest and
/// tokio's files need one, and the main loop is GLib's.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("beacon-worker")
            .build()
            .expect("the system can start threads")
    })
}

impl AppSender {
    pub fn new() -> (Self, async_channel::Receiver<Event>) {
        let (send, receive) = async_channel::unbounded();
        (Self(send), receive)
    }

    /// Handle `message` after what the window is doing now.
    pub fn input(&self, message: AppMsg) {
        // Unbounded: it fails only once the window is gone.
        let _ = self.0.send_blocking(Event::Message(message));
    }

    /// Hand `result` to the window, from work still under way.
    pub fn command(&self, result: AppCommand) {
        let _ = self.0.try_send(Event::Command(result));
    }

    /// Run `work` on the runtime and hand its result to the window.
    pub fn spawn(&self, work: impl Future<Output = AppCommand> + Send + 'static) {
        let send = self.0.clone();
        runtime().spawn(async move {
            let _ = send.send(Event::Command(work.await)).await;
        });
    }

    /// Run blocking `work` (the database) off the main thread and hand its
    /// result to the window.
    pub fn spawn_blocking(&self, work: impl FnOnce() -> AppCommand + Send + 'static) {
        let send = self.0.clone();
        runtime().spawn_blocking(move || {
            let _ = send.send_blocking(Event::Command(work()));
        });
    }
}
