// SPDX-License-Identifier: GPL-3.0-or-later

//! Logging in and out from the Sync page: Login Flow v2 through the
//! browser, or a password turned into an app password; the app password
//! goes to the keyring and the account to the library.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use beacon_core::SyncError;
use beacon_core::services::http_client;
use beacon_core::store::SyncAccount;
use beacon_core::sync::keyring;
use beacon_core::sync::nextcloud::{self, BrowserLogin, Credentials, NextcloudBackend};
use gettextrs::gettext;
use gtk::gio;
use gtk::prelude::*;

use super::{LoggingIn, POLL_EVERY, SyncCommand, SyncMsg, command};
use crate::app::library::Parts;
use crate::app::{App, AppMsg, toast};
use crate::ui::dialogs::sync_page::SyncPageOutput;

impl App {
    pub(super) fn sync_page_output(&mut self, output: SyncPageOutput) {
        match output {
            SyncPageOutput::LogInWithBrowser { server } => {
                let Some(server) = self.parse_server(&server) else {
                    return;
                };
                self.start_login(true);
                self.sender.spawn(async move {
                    let result = nextcloud::start_browser_login(&http_client(), &server).await;
                    command(SyncCommand::BrowserLogin(result))
                });
            }
            SyncPageOutput::LogInWithPassword {
                server,
                user,
                password,
            } => {
                let Some(server) = self.parse_server(&server) else {
                    return;
                };
                self.start_login(false);
                let user = user.trim().to_owned();
                self.finish_login(async move {
                    nextcloud::log_in_with_password(&http_client(), &server, &user, &password).await
                });
            }
            SyncPageOutput::CancelLogIn => {
                if let Some(login) = self.sync.logging_in.take() {
                    login.cancel.store(true, Ordering::Relaxed);
                }
                self.show_sync();
            }
            SyncPageOutput::LogOut => self.log_out(),
            SyncPageOutput::SetActive(active) => {
                let Some(account) = &mut self.sync.account else {
                    return;
                };
                account.active = active;
                self.show_sync();
                self.job(Parts::NONE, false, move |store| {
                    store.set_sync_active(active)
                });
                if active {
                    self.request_sync(true);
                }
            }
            SyncPageOutput::SyncNow => self.request_sync(true),
        }
    }

    fn parse_server(&self, server: &str) -> Option<url::Url> {
        match nextcloud::server_url(server) {
            Ok(url) => Some(url),
            Err(err) => {
                self.sync_failed(&err, true);
                None
            }
        }
    }

    fn start_login(&mut self, browser: bool) {
        self.sync.logging_in = Some(LoggingIn {
            browser,
            cancel: Arc::default(),
        });
        self.show_sync();
    }

    /// Open the login page in the browser (the OpenURI portal), then wait
    /// for the user to approve it there.
    pub(super) fn browser_login(&mut self, result: Result<BrowserLogin, SyncError>) {
        let Some(cancel) = self.sync.logging_in.as_ref().map(|l| l.cancel.clone()) else {
            return;
        };
        let login = match result {
            Ok(login) => login,
            Err(err) => {
                self.logged_in(Err(err));
                return;
            }
        };
        let window = self
            .widgets
            .toasts
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        let sender = self.sender.clone();
        gtk::UriLauncher::new(login.login.as_str()).launch(
            window.as_ref(),
            gio::Cancellable::NONE,
            move |result| {
                if let Err(err) = result {
                    tracing::warn!("could not open the login page: {err}");
                    sender.input(AppMsg::Sync(SyncMsg::Page(SyncPageOutput::CancelLogIn)));
                }
            },
        );
        self.finish_login(async move {
            nextcloud::wait_for_browser_login(&http_client(), &login, POLL_EVERY, &cancel).await
        });
    }

    /// Once `login` has the app password: keep it in the keyring and the
    /// account in the library.
    fn finish_login(
        &self,
        login: impl Future<Output = Result<Credentials, SyncError>> + Send + 'static,
    ) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let label = gettext("Beacon Sync Account");
        self.sender.spawn(async move {
            let result = async {
                let credentials = login.await?;
                keyring::save(&credentials, &label).await?;
                let account = library.with_store(|store| {
                    store.set_sync_account(&credentials.server, &credentials.user)?;
                    store.sync_account()
                })?;
                let account = account.ok_or(SyncError::NotConfigured)?;
                Ok((account, credentials))
            }
            .await;
            command(SyncCommand::LoggedIn(result))
        });
    }

    pub(super) fn logged_in(&mut self, result: Result<(SyncAccount, Credentials), SyncError>) {
        // Cancelled meanwhile: forget it.
        if self.sync.logging_in.take().is_none() {
            return;
        }
        match result {
            Ok((account, credentials)) => {
                toast(
                    &self.widgets.toasts,
                    &gettext("Logged in as {user}").replace("{user}", &account.user),
                );
                self.sync.account = Some(account);
                self.sync.backend = Some(NextcloudBackend::new(http_client(), credentials));
                self.show_sync();
                // The first sync merges both sides.
                self.request_sync(true);
            }
            Err(err) => {
                self.show_sync();
                self.sync_failed(&err, true);
            }
        }
    }

    /// Log Out: revoke the app password, then forget the account and the
    /// keyring item even if that fails, as Nextcloud's docs advise.
    fn log_out(&mut self) {
        let backend = self.sync.backend.take();
        self.sync.account = None;
        if let Some((source, _)) = self.sync.timer.take() {
            source.remove();
        }
        self.show_sync();
        let Some(library) = self.library.clone() else {
            return;
        };
        self.sender.spawn(async move {
            if let Some(backend) = backend
                && let Err(err) = nextcloud::revoke(&http_client(), backend.credentials()).await
            {
                tracing::warn!("could not revoke the app password: {err}");
            }
            let forgotten = keyring::forget().await;
            let removed = library.with_store(|store| store.remove_sync_account());
            command(SyncCommand::LoggedOut(
                removed.map_err(SyncError::from).and(forgotten),
            ))
        });
    }
}
