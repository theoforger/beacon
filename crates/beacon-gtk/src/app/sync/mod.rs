// SPDX-License-Identifier: GPL-3.0-or-later

//! Sync with nextcloud-gpodder (`docs/specs/sync.md`): logging in and out
//! from the Sync page of Preferences, and syncing after refreshes, after
//! subscribing and unsubscribing, a while after playback pauses, on Sync
//! Now and once when the window closes. The work runs in the background
//! through `beacon_core::sync`; this keeps the account and the page in
//! step.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use adw::prelude::*;
use beacon_core::SyncError;
use beacon_core::services::http_client;
use beacon_core::services::library::Library;
use beacon_core::store::SyncAccount;
use beacon_core::sync::keyring;
use beacon_core::sync::nextcloud::{BrowserLogin, Credentials, NextcloudBackend};
use beacon_core::sync::service::{SyncService, SyncSummary};
use gtk::glib;

use super::library::Parts;
use super::{App, AppCommand, AppMsg, toast};
use crate::ui::dialogs::sync_page::{SyncPage, SyncPageInput, SyncPageOutput, SyncView};
use crate::ui::errors;

/// How long after playback pauses or stops a sync runs, so a pause to
/// answer the door does not sync each time.
pub(super) const AFTER_PAUSE: Duration = Duration::from_secs(30);
/// After subscribing or unsubscribing: long enough for the change to be
/// written first.
pub(super) const AFTER_SUBSCRIBING: Duration = Duration::from_secs(1);
/// How long closing the window waits for its last sync.
const ON_CLOSE: Duration = Duration::from_secs(5);
mod login;

/// How often a browser login asks whether it was approved.
const POLL_EVERY: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum SyncMsg {
    Page(SyncPageOutput),
    /// A sync waited for (after a pause, say) is due.
    Due,
    /// The window is closing: one last sync, then close.
    Closing,
    /// Log Out, confirmed in its dialog.
    ConfirmLogOut,
}

/// The account read at launch, with its app password or why the keyring
/// could not give it.
type LoadedAccount =
    Result<Option<(SyncAccount, Result<Option<Credentials>, SyncError>)>, SyncError>;

#[derive(Debug)]
pub enum SyncCommand {
    /// The account and its app password, read at launch.
    /// The account and its app password, read at launch. The account
    /// stands even when the keyring cannot be read.
    Loaded(LoadedAccount),
    /// A sync ended; `manual` for Sync Now and the first one after login.
    Synced {
        result: Result<SyncSummary, SyncError>,
        account: Option<SyncAccount>,
        manual: bool,
    },
    BrowserLogin(Result<BrowserLogin, SyncError>),
    LoggedIn(Result<(SyncAccount, Credentials), SyncError>),
    LoggedOut(Result<(), SyncError>),
    /// Log Out was asked for: whether changes wait to be sent, which it
    /// would drop.
    LogOutAsked(bool),
    /// The sync on closing ended, or ran out of time.
    Closed,
}

struct LoggingIn {
    browser: bool,
    cancel: Arc<AtomicBool>,
}

/// The sync side of the window.
#[derive(Default)]
pub struct Sync {
    service: Option<Arc<SyncService>>,
    account: Option<SyncAccount>,
    backend: Option<NextcloudBackend>,
    /// The account has been read at launch.
    loaded: bool,
    /// A sync was asked for before that.
    wanted: bool,
    running: bool,
    /// Asked for again while one ran: `Some(manual)`.
    again: Option<bool>,
    logging_in: Option<LoggingIn>,
    /// A sync waiting to run, and when.
    timer: Option<(glib::SourceId, Instant)>,
    page: glib::WeakRef<SyncPage>,
    /// Whether closing waits for a last sync; read by the close handler.
    on_close: Rc<Cell<bool>>,
    /// The last sync before closing has started.
    closing: bool,
    /// The keyring could not be read at launch.
    keyring_failed: bool,
}

impl Sync {
    pub(super) fn new(library: Option<&Arc<Library>>) -> Self {
        Self {
            service: library.map(|library| Arc::new(SyncService::new(library.clone()))),
            ..Self::default()
        }
    }

    /// Shared with the window's close handler.
    pub(super) fn on_close(&self) -> Rc<Cell<bool>> {
        self.on_close.clone()
    }

    fn ready(&self) -> Option<(Arc<SyncService>, NextcloudBackend)> {
        self.account.as_ref().filter(|account| account.active)?;
        Some((self.service.clone()?, self.backend.clone()?))
    }

    /// Take in the account read at launch. Returns what to report: the
    /// keyring failing leaves the account signed in, without its password.
    fn apply_loaded(&mut self, result: LoadedAccount) -> Option<SyncError> {
        self.loaded = true;
        let (account, credentials) = match result {
            Ok(Some(loaded)) => loaded,
            Ok(None) => return None,
            Err(err) => return Some(err),
        };
        self.account = Some(account);
        match credentials {
            Ok(credentials) => {
                self.backend = credentials.map(|c| NextcloudBackend::new(http_client(), c));
                None
            }
            Err(err) => {
                self.keyring_failed = true;
                Some(err)
            }
        }
    }

    /// Why a signed-in account cannot sync: its password is missing.
    fn missing_password(&self) -> SyncError {
        if self.keyring_failed {
            SyncError::Keyring("the keyring could not be read at launch".to_owned())
        } else {
            SyncError::Unauthorized
        }
    }

    fn view(&self) -> SyncView {
        match (&self.account, &self.logging_in) {
            (Some(account), _) => SyncView::SignedIn {
                user: account.user.clone(),
                server: account.server.to_string(),
                active: account.active,
                last_synced: account.last_synced,
                syncing: self.running,
            },
            (None, Some(login)) => SyncView::LoggingIn {
                browser: login.browser,
            },
            (None, None) => SyncView::SignedOut,
        }
    }
}

impl App {
    /// Read the account and its app password, at launch.
    pub(super) fn load_sync(&self) {
        let Some(library) = self.library.clone() else {
            return;
        };
        self.sender.spawn(async move {
            let result = async {
                let Some(account) = library.with_store(|store| store.sync_account())? else {
                    return Ok(None);
                };
                let credentials = keyring::load(&account.server, &account.user).await;
                Ok(Some((account, credentials)))
            }
            .await;
            command(SyncCommand::Loaded(result))
        });
    }

    /// The Sync page for Preferences, showing the account as it is now.
    pub(super) fn sync_page(&mut self) -> SyncPage {
        let page = SyncPage::default();
        let sender = self.sender.clone();
        page.connect_output(move |output| sender.input(AppMsg::Sync(SyncMsg::Page(output))));
        page.send(SyncPageInput::Show(self.sync.view()));
        self.sync.page.set(Some(&page));
        page
    }

    fn show_sync(&self) {
        let sync = &self.sync;
        sync.on_close.set(sync.ready().is_some());
        if let Some(page) = sync.page.upgrade() {
            page.send(SyncPageInput::Show(sync.view()));
        }
    }

    pub(super) fn sync_message(&mut self, message: SyncMsg) {
        match message {
            SyncMsg::Page(output) => self.sync_page_output(output),
            SyncMsg::Due => {
                self.sync.timer = None;
                self.request_sync(false);
            }
            SyncMsg::Closing => self.sync_on_close(),
            SyncMsg::ConfirmLogOut => self.log_out(),
        }
    }

    /// Sync now, or once the one running ends. `manual` (Sync Now) shows
    /// every error; otherwise only those the user can fix.
    pub(super) fn request_sync(&mut self, manual: bool) {
        let sync = &mut self.sync;
        if !sync.loaded {
            sync.wanted = true;
            return;
        }
        if sync.running {
            sync.again = Some(sync.again.unwrap_or(false) || manual);
            return;
        }
        if sync.account.as_ref().is_some_and(|a| a.active) && sync.backend.is_none() {
            // Signed in, but without the app password. Launch already said
            // so; only Sync Now says it again.
            let error = sync.missing_password();
            if manual {
                self.sync_failed(&error, true);
            } else {
                tracing::debug!("not syncing: {error}");
            }
            return;
        }
        let Some((service, backend)) = sync.ready() else {
            return;
        };
        sync.running = true;
        self.show_sync();
        self.sender.spawn(async move {
            let result = service.sync(&backend).await;
            let account = service
                .library()
                .with_store(|store| store.sync_account())
                .ok()
                .flatten();
            command(SyncCommand::Synced {
                result,
                account,
                manual,
            })
        });
    }

    /// Sync in `delay`, unless one is due sooner already.
    pub(super) fn schedule_sync(&mut self, delay: Duration) {
        if self.sync.ready().is_none() {
            return;
        }
        let due = Instant::now() + delay;
        if let Some((_, at)) = &self.sync.timer
            && *at <= due
        {
            return;
        }
        if let Some((source, _)) = self.sync.timer.take() {
            source.remove();
        }
        let sender = self.sender.clone();
        let source = glib::timeout_add_local_once(delay, move || {
            sender.input(AppMsg::Sync(SyncMsg::Due));
        });
        self.sync.timer = Some((source, due));
    }

    fn sync_failed(&self, error: &SyncError, manual: bool) {
        tracing::warn!("sync failed: {error}");
        if (manual || errors::sync_needs_attention(error))
            && let Some(message) = errors::sync(error)
        {
            self.sync_toast(&message);
        }
    }

    /// Show `message` in Preferences while it is open, or else in the
    /// window: a narrow window's Preferences is a bottom sheet over the
    /// window's toasts.
    pub(super) fn sync_toast(&self, message: &str) {
        let dialog = self
            .sync
            .page
            .upgrade()
            .filter(WidgetExt::is_mapped)
            .and_then(|page| page.ancestor(adw::PreferencesDialog::static_type()))
            .and_then(|dialog| dialog.downcast::<adw::PreferencesDialog>().ok());
        match dialog {
            Some(dialog) => dialog.add_toast(adw::Toast::new(message)),
            None => toast(&self.widgets.toasts, message),
        }
    }

    pub(super) fn sync_command(&mut self, command: SyncCommand) {
        match command {
            SyncCommand::Loaded(result) => {
                if let Some(err) = self.sync.apply_loaded(result) {
                    self.sync_failed(&err, false);
                }
                self.show_sync();
                if std::mem::take(&mut self.sync.wanted) {
                    self.request_sync(false);
                }
            }
            SyncCommand::Synced {
                result,
                account,
                manual,
            } => {
                self.sync.running = false;
                // Signed out meanwhile: the account stays gone.
                if self.sync.account.is_some() && account.is_some() {
                    self.sync.account = account;
                }
                match result {
                    Ok(summary) => {
                        for (feed, err) in &summary.failed {
                            tracing::warn!("could not subscribe to {feed} from the server: {err}");
                        }
                        tracing::debug!(?summary, "synced");
                        if summary.changed_library() {
                            self.reload(Parts::ALL);
                        }
                    }
                    Err(SyncError::NotConfigured) => {}
                    Err(err) => self.sync_failed(&err, manual),
                }
                self.show_sync();
                if let Some(manual) = self.sync.again.take() {
                    self.request_sync(manual);
                }
            }
            SyncCommand::BrowserLogin(result) => self.browser_login(result),
            SyncCommand::LoggedIn(result) => self.logged_in(result),
            SyncCommand::LogOutAsked(pending) => {
                if pending {
                    self.confirm_log_out();
                } else {
                    self.log_out();
                }
            }
            SyncCommand::LoggedOut(result) => {
                if let Err(err) = result {
                    self.sync_failed(&err, true);
                }
            }
            SyncCommand::Closed => {
                if let Some(window) = self
                    .widgets
                    .toasts
                    .root()
                    .and_then(|root| root.downcast::<gtk::Window>().ok())
                {
                    window.close();
                }
            }
        }
    }

    /// The window is closing: pause, sync once, then close, after at most
    /// five seconds.
    fn sync_on_close(&mut self) {
        if let Some(player) = &self.player {
            player.pause();
        }
        // The second time, when time runs out, closes at once.
        let first = !std::mem::replace(&mut self.sync.closing, true);
        let Some((service, backend)) = self.sync.ready().filter(|_| first) else {
            self.sync_command(SyncCommand::Closed);
            return;
        };
        self.sender.spawn(async move {
            if let Err(err) = service.sync(&backend).await {
                tracing::warn!("could not sync on closing: {err}");
            }
            command(SyncCommand::Closed)
        });
        let sender = self.sender.clone();
        glib::timeout_add_local_once(ON_CLOSE, move || {
            sender.input(AppMsg::Sync(SyncMsg::Closing));
        });
    }
}

fn command(command: SyncCommand) -> AppCommand {
    AppCommand::Sync(Box::new(command))
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::*;

    fn account() -> SyncAccount {
        SyncAccount {
            server: Url::parse("https://cloud.example.com/").unwrap(),
            user: "ada".to_owned(),
            active: true,
            subscriptions_since: None,
            actions_since: None,
            last_synced: None,
        }
    }

    #[test]
    fn an_unreadable_keyring_leaves_the_account_signed_in() {
        let mut sync = Sync::default();
        let error = sync.apply_loaded(Ok(Some((
            account(),
            Err(SyncError::Keyring("locked".to_owned())),
        ))));
        assert!(matches!(error, Some(SyncError::Keyring(_))));
        assert!(matches!(sync.view(), SyncView::SignedIn { .. }));
        assert!(sync.ready().is_none(), "nothing to sync with");
        assert!(matches!(sync.missing_password(), SyncError::Keyring(_)));
    }

    #[test]
    fn a_missing_password_asks_to_log_in_again() {
        let mut sync = Sync::default();
        assert!(sync.apply_loaded(Ok(Some((account(), Ok(None))))).is_none());
        assert!(matches!(sync.view(), SyncView::SignedIn { .. }));
        assert!(matches!(sync.missing_password(), SyncError::Unauthorized));
    }

    #[test]
    fn no_account_is_signed_out() {
        let mut sync = Sync::default();
        assert!(sync.apply_loaded(Ok(None)).is_none());
        assert_eq!(sync.view(), SyncView::SignedOut);
    }
}
