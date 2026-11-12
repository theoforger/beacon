// SPDX-License-Identifier: GPL-3.0-or-later

//! A podcast page's widgets: the header (Search, Filter, More Options and
//! the details toggle; a preview has More Options and the toggle only),
//! the search bar, the hero and the list, and their actions.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use super::{PREVIEW_TAG, PageKind, Parts, SHOW_TAG, ShowPage, ShowPageInput, ShowPageOutput};
use crate::ui::episode_list::{EpisodeList, Folding};
use crate::ui::page_filters;
use crate::ui::selection::Selection;
use crate::ui::selection_bar;
use crate::ui::show_hero::ShowHero;
use crate::ui::show_search::ShowSearch;
use crate::ui::widgets::episode_row::RowSetup;

impl ShowPage {
    /// A page of `kind`, with `details_toggle`, its details toggle, which
    /// the content pane keeps in step.
    pub fn new(details_toggle: &gtk::ToggleButton, kind: PageKind) -> Self {
        let library = kind == PageKind::Library;
        let page: Self = glib::Object::builder()
            .property("tag", if library { SHOW_TAG } else { PREVIEW_TAG })
            // Until the show's own title arrives: the page is pushed first,
            // and a page needs a title for Back and screen readers.
            .property("title", gettext("Podcast"))
            .build();
        let output = page.imp().output.clone();
        let input = {
            let page = page.downgrade();
            move |message: ShowPageInput| {
                if let Some(page) = page.upgrade() {
                    page.send(message);
                }
            }
        };
        // The page is the show's own: its name and cover would repeat on
        // every row. A preview's rows only show: their episodes are not in
        // the library to play, open or change.
        let rows = RowSetup {
            on_action: Rc::new({
                let output = output.clone();
                move |item, action| {
                    if library {
                        output.emit(ShowPageOutput::Action(Box::new(item.clone()), action));
                    }
                }
            }),
            show_identity: false,
            selection: Selection::default(),
            selectable: library,
            actions: library,
        };
        let hero = ShowHero::new();
        hero.set_subscribed(library);
        let folding = Folding {
            folded: Rc::default(),
            changed: Rc::new({
                let input = input.clone();
                move || input(ShowPageInput::Refold)
            }),
        };
        // Translators: how a screen reader names a show's episode list.
        let list = EpisodeList::new(
            &rows,
            &gettext("Episodes"),
            Some(hero.widget().clone().upcast()),
            Some(folding.clone()),
        );
        let more = super::menu::more_button();
        if !library {
            more.set_menu_model(Some(&super::menu::preview_menu()));
        }

        let header = adw::HeaderBar::builder().show_title(false).build();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(list.widget()));
        page.set_child(Some(&toolbar));
        // The details toggle at the far end, as on every page; then the
        // show's More Options menu, which the HIG puts in the header bar
        // ("Secondary menus are located in the header bar"), as GNOME
        // Podcasts does; then sort and filter, Search and Select.
        header.pack_end(details_toggle);
        header.pack_end(&more);
        let (filter, search) = if library {
            let (filter, search) = page.library_header(&header, &toolbar, &rows);
            (Some(filter), Some(search))
        } else {
            (None, None)
        };
        let title_check =
            crate::ui::show_title::follow_scroll(&header, list.widget(), hero.title());

        let actions = gio::SimpleActionGroup::new();
        let action = |name: &str, message: fn() -> ShowPageInput| {
            let action = gio::SimpleAction::new(name, None);
            let input = input.clone();
            action.connect_activate(move |_, _| input(message()));
            actions.add_action(&action);
            action
        };
        action("mark-all", || ShowPageInput::MarkAll);
        let download_all = action("download-all", || ShowPageInput::DownloadAll);
        action("copy-feed", || ShowPageInput::CopyFeedAddress);
        let view_in_store = action("view-in-store", || ShowPageInput::ViewInStore);
        page.insert_action_group("show", Some(&actions));
        let i = input.clone();
        hero.unsubscribe
            .connect_clicked(move |_| i(ShowPageInput::Unsubscribe));
        hero.subscribe
            .connect_clicked(move |_| input(ShowPageInput::Subscribe));

        let _ = page.imp().parts.set(Parts {
            kind,
            hero,
            filter,
            more,
            download_all,
            view_in_store,
            rows,
            list,
            folding,
            search,
            title_check,
        });
        page
    }

    /// A subscribed show's header: Filter, Search with its bar, which
    /// typing opens, and selection mode.
    fn library_header(
        &self,
        header: &adw::HeaderBar,
        toolbar: &adw::ToolbarView,
        rows: &RowSetup,
    ) -> (page_filters::FilterButton, ShowSearch) {
        let output = self.imp().output.clone();
        let page = self.downgrade();
        let filter = page_filters::FilterButton::new({
            let page = page.clone();
            move |choice| {
                if let Some(page) = page.upgrade() {
                    page.send(ShowPageInput::Choose(choice));
                }
            }
        });
        let search = ShowSearch::new({
            let (output, page) = (output.clone(), page.clone());
            move |query| {
                let show = page
                    .upgrade()
                    .and_then(|p| p.imp().data.borrow().as_ref().map(|d| d.show.id));
                if let Some(show) = show {
                    output.emit(ShowPageOutput::Search(show, query));
                }
            }
        });
        toolbar.add_top_bar(&search.bar);
        header.pack_end(filter.widget());
        header.pack_end(&search.toggle);
        let select = selection_bar::select_button(&rows.selection);
        header.pack_end(&selection_bar::select_holder(&select));
        selection_bar::attach(
            toolbar,
            header,
            &select,
            &rows.selection,
            Box::new(move |ids, change| output.emit(ShowPageOutput::Change(ids, change))),
        );
        self.capture_typing(&search.bar, rows);
        // Leaving the page leaves selection mode and the search.
        let selection = rows.selection.clone();
        let bar = search.bar.clone();
        self.connect_hidden(move |_| {
            selection.set_active(false);
            bar.set_search_mode(false);
        });
        (filter, search)
    }

    /// Type to search while the page shows, as on the root page and in
    /// GNOME Podcasts (`show.rs`), except in selection mode. The window
    /// catches the typing, as the root page's does: the page alone lost the
    /// second letter while the field took focus. The page is on screen from
    /// when it maps until it starts to leave: during Back's slide it is
    /// still mapped, and leaving selection mode then gave it the typing back.
    fn capture_typing(&self, bar: &gtk::SearchBar, rows: &RowSetup) {
        let on_screen = Rc::new(Cell::new(false));
        let capture = Rc::new({
            let (bar, page) = (bar.clone(), self.downgrade());
            let (selecting, on_screen) = (rows.selection.clone(), on_screen.clone());
            move || {
                let window = page
                    .upgrade()
                    .and_then(|page| page.root())
                    .filter(|_| on_screen.get() && !selecting.is_active());
                bar.set_key_capture_widget(window.as_ref());
            }
        });
        let showing = |on: bool| {
            let (on_screen, capture) = (on_screen.clone(), capture.clone());
            move |_: &ShowPage| {
                on_screen.set(on);
                capture();
            }
        };
        self.connect_map(showing(true));
        self.connect_hiding(showing(false));
        self.connect_unmap(showing(false));
        rows.selection.connect_changed(move |_| capture());
    }
}
