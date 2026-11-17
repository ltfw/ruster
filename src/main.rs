use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Button, Entry, HeaderBar, Label, Orientation,
    Stack,
};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use webkit2gtk::{WebView, WebViewExt};

const NEW_TAB_HTML: &str = r#"<!doctype html>
<html lang="id"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>New tab</title>
<style>
*{box-sizing:border-box}body{margin:0;background:#f4f5f7;color:#20242b;font:15px system-ui,sans-serif}
main{min-height:calc(100vh - 40px);display:grid;place-content:center;justify-items:center;padding:32px}
.mark{width:44px;height:44px;border-radius:14px;background:#29483b;color:#f5f1e7;display:grid;place-items:center;font:600 30px Georgia,serif}
h1{margin:22px 0 8px;font-size:27px;font-weight:550;letter-spacing:-.7px}.hint{margin:0 0 26px;color:#737b87;font-size:14px}
form{width:min(620px,82vw);height:50px;padding:4px 5px 4px 15px;display:flex;align-items:center;gap:10px;border:1px solid #d6dae1;border-radius:15px;background:#fff;box-shadow:0 3px 15px #20242b0c}
input{min-width:0;flex:1;border:0;outline:0;color:#20242b;background:transparent;font:14px system-ui,sans-serif}button{height:38px;padding:0 15px;border:0;border-radius:11px;background:#29483b;color:#fff;font:13px system-ui,sans-serif;cursor:pointer}button:hover{background:#20392f}
kbd{margin-top:18px;color:#959ba5;font:11px ui-monospace,monospace}
</style></head><body><main><div class="mark">r</div><h1>Mau membuka apa?</h1>
<p class="hint">Ketik alamat situs atau kata pencarian di bilah atas.</p>
<form id="search"><input id="query" aria-label="Cari di web" placeholder="Cari di web…" autocomplete="off"><button type="submit">Cari</button></form>
<kbd>CTRL + L UNTUK PINDAH KE BILAH ALAMAT</kbd></main>
<script>document.querySelector('#search').addEventListener('submit',e=>{e.preventDefault();const q=document.querySelector('#query').value.trim();if(q)location.href='https://www.google.com/search?q='+encodeURIComponent(q)})</script></body></html>"#;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Bookmark {
    title: String,
    url: String,
}

struct Browser {
    window: ApplicationWindow,
    stack: Stack,
    tab_bar: GtkBox,
    address: Entry,
    status: Label,
    bookmarks: Rc<RefCell<Vec<Bookmark>>>,
    bookmark_bar: GtkBox,
    next_tab_id: Rc<Cell<u64>>,
}

#[derive(Clone)]
struct BrowserHandles {
    window: glib::WeakRef<ApplicationWindow>,
    stack: glib::WeakRef<Stack>,
    tab_bar: glib::WeakRef<GtkBox>,
    address: glib::WeakRef<Entry>,
    status: glib::WeakRef<Label>,
    bookmark_bar: glib::WeakRef<GtkBox>,
    bookmarks: Rc<RefCell<Vec<Bookmark>>>,
    next_tab_id: Rc<Cell<u64>>,
}

impl Browser {
    fn new_tab(&self, url: Option<&str>) {
        self.handles().new_tab(url);
    }

    fn handles(&self) -> BrowserHandles {
        BrowserHandles {
            window: self.window.downgrade(),
            stack: self.stack.downgrade(),
            tab_bar: self.tab_bar.downgrade(),
            address: self.address.downgrade(),
            status: self.status.downgrade(),
            bookmark_bar: self.bookmark_bar.downgrade(),
            bookmarks: self.bookmarks.clone(),
            next_tab_id: self.next_tab_id.clone(),
        }
    }
}

impl BrowserHandles {
    fn current_view(&self) -> Option<WebView> {
        self.stack
            .upgrade()?
            .visible_child()?
            .downcast::<WebView>()
            .ok()
    }

    fn toggle_bookmark(&self) {
        let Some(view) = self.current_view() else {
            return;
        };
        let Some(url) = view.uri() else {
            return;
        };
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return;
        }
        let title = view
            .title()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| url.clone());
        let mut saved = self.bookmarks.borrow_mut();
        if let Some(index) = saved.iter().position(|item| item.url == url) {
            saved.remove(index);
            if let Some(status) = self.status.upgrade() {
                status.set_text("Bookmark removed");
            }
        } else {
            saved.push(Bookmark {
                title: title.to_string(),
                url: url.to_string(),
            });
            if let Some(status) = self.status.upgrade() {
                status.set_text("Bookmark saved");
            }
        }
        let _ = save_bookmarks(&saved);
        if let (Some(bar), Some(address)) = (self.bookmark_bar.upgrade(), self.address.upgrade()) {
            render_bookmarks(&bar, &saved, &address);
        }
    }

    fn new_tab(&self, url: Option<&str>) {
        let Some(stack) = self.stack.upgrade() else {
            return;
        };
        let Some(tab_bar) = self.tab_bar.upgrade() else {
            return;
        };
        let view = WebView::new();
        let id = format!("tab-{}", self.next_tab_id.get());
        self.next_tab_id.set(self.next_tab_id.get() + 1);
        view.set_hexpand(true);
        view.set_vexpand(true);
        // GtkStack ignores set_visible_child for hidden children, so show the view first.
        view.show();
        stack.add_named(&view, &id);
        stack.set_visible_child(&view);
        if let Some(url) = url {
            view.load_uri(url);
        } else {
            view.load_html(NEW_TAB_HTML, Some("about:blank"));
        }
        let window = self.window.upgrade();
        let (label, _) = add_tab_item(&stack, &tab_bar, window.as_ref(), &view, &id);
        update_tab_selection(&stack, &tab_bar);
        watch_view(&view, &label, self.clone());
    }

    fn close_current_tab(&self) {
        let (Some(stack), Some(tab_bar), Some(view)) = (
            self.stack.upgrade(),
            self.tab_bar.upgrade(),
            self.current_view(),
        ) else {
            return;
        };
        let window = self.window.upgrade();
        close_tab(&stack, &tab_bar, window.as_ref(), &view);
    }
}

fn is_active_tab(handles: &BrowserHandles, view: &WebView) -> bool {
    let Some(stack) = handles.stack.upgrade() else {
        return false;
    };
    stack
        .visible_child()
        .is_some_and(|active| active == view.clone().upcast::<gtk::Widget>())
}

fn watch_view(view: &WebView, tab_label: &Label, handles: BrowserHandles) {
    let title_label = tab_label.clone();
    let title_handles = handles.clone();
    view.connect_title_notify(move |view| {
        let title = view
            .title()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "New tab".into());
        title_label.set_text(&title.chars().take(22).collect::<String>());
        if is_active_tab(&title_handles, view) {
            if let Some(window) = title_handles.window.upgrade() {
                window.set_title(&format!("{title} — Ruster"));
            }
        }
    });

    let uri_handles = handles.clone();
    view.connect_uri_notify(move |view| {
        if is_active_tab(&uri_handles, view) {
            if let Some(address) = uri_handles.address.upgrade() {
                let uri = view.uri().unwrap_or_default();
                address.set_text(if uri.as_str() == "about:blank" {
                    ""
                } else {
                    uri.as_str()
                });
            }
        }
        if is_active_tab(&uri_handles, view) {
            if let Some(status) = uri_handles.status.upgrade() {
                status.set_text(
                    if view
                        .uri()
                        .as_deref()
                        .is_some_and(|uri| uri.starts_with("https://"))
                    {
                        "Secure connection"
                    } else if view.uri().as_deref() == Some("about:blank") {
                        "New tab — ready to search"
                    } else {
                        "Not secure"
                    },
                );
            }
        }
    });

    let load_handles = handles.clone();
    view.connect_load_changed(move |view, event| {
        if is_active_tab(&load_handles, view) {
            if let Some(status) = load_handles.status.upgrade() {
                match event {
                    webkit2gtk::LoadEvent::Started => status.set_text("Loading…"),
                    webkit2gtk::LoadEvent::Finished => status.set_text(
                        if view
                            .uri()
                            .as_deref()
                            .is_some_and(|uri| uri.starts_with("https://"))
                        {
                            "Secure connection"
                        } else if view.uri().as_deref() == Some("about:blank") {
                            "New tab — ready to search"
                        } else {
                            "Ready"
                        },
                    ),
                    _ => {}
                }
            }
        }
    });

    let failure_handles = handles.clone();
    view.connect_load_failed(move |view, _event, failing_uri, error| {
        eprintln!("Ruster: failed to load {failing_uri}: {error}");
        if is_active_tab(&failure_handles, view) {
            if let Some(status) = failure_handles.status.upgrade() {
                status.set_text(&format!("Could not load page: {}", error.message()));
            }
        }
        let message = error.message().to_ascii_lowercase();
        error.matches(webkit2gtk::NetworkError::Cancelled)
            || message.contains("cancelled")
            || message.contains("canceled")
    });

    view.connect_create(move |source, _action| {
        let stack = handles.stack.upgrade()?;
        let tab_bar = handles.tab_bar.upgrade()?;
        let new_view = WebView::with_related_view(source);
        let window = handles.window.upgrade();
        let id = format!("tab-{}", handles.next_tab_id.get());
        handles.next_tab_id.set(handles.next_tab_id.get() + 1);
        new_view.set_hexpand(true);
        new_view.set_vexpand(true);
        new_view.show();
        stack.add_named(&new_view, &id);
        stack.set_visible_child(&new_view);
        let (label, _) = add_tab_item(&stack, &tab_bar, window.as_ref(), &new_view, &id);
        update_tab_selection(&stack, &tab_bar);
        watch_view(&new_view, &label, handles.clone());
        Some(new_view.upcast())
    });
}

fn add_tab_item(
    stack: &Stack,
    tab_bar: &GtkBox,
    window: Option<&ApplicationWindow>,
    view: &WebView,
    page_name: &str,
) -> (Label, Button) {
    let tab = GtkBox::new(Orientation::Horizontal, 2);
    let title = Label::new(Some("New tab"));
    let select = Button::new();
    select.set_relief(gtk::ReliefStyle::None);
    select.set_size_request(130, 38);
    select.add(&title);
    select.style_context().add_class("tab-switch");
    let close = Button::with_label("×");
    close.set_relief(gtk::ReliefStyle::None);
    close.set_focus_on_click(false);
    close.set_tooltip_text(Some("Close tab"));
    close.style_context().add_class("tab-close");
    tab.pack_start(&select, true, true, 0);
    tab.pack_start(&close, false, false, 0);

    let tab_bar_ref = tab_bar.downgrade();
    let stack_ref = stack.downgrade();
    let page_name = page_name.to_owned();
    select.connect_clicked(move |_| {
        if let (Some(stack), Some(tab_bar)) = (stack_ref.upgrade(), tab_bar_ref.upgrade()) {
            stack.set_visible_child_name(&page_name);
            update_tab_selection(&stack, &tab_bar);
        }
    });

    let stack_ref = stack.downgrade();
    let tab_bar_ref = tab_bar.downgrade();
    let window_ref = window.map(|window| window.downgrade());
    let view = view.clone();
    close.connect_clicked(move |_| {
        if let (Some(stack), Some(tab_bar)) = (stack_ref.upgrade(), tab_bar_ref.upgrade()) {
            let window = window_ref.as_ref().and_then(|weak| weak.upgrade());
            close_tab(&stack, &tab_bar, window.as_ref(), &view);
        }
    });
    tab_bar.pack_start(&tab, false, false, 0);
    tab_bar.show_all();
    update_tab_selection(stack, tab_bar);
    (title, select)
}

fn update_tab_selection(stack: &Stack, tab_bar: &GtkBox) {
    let Some(active_page) = stack.visible_child() else {
        return;
    };
    for (page, item) in stack.children().into_iter().zip(tab_bar.children()) {
        let Ok(item) = item.downcast::<GtkBox>() else {
            continue;
        };
        let Some(button) = item
            .children()
            .into_iter()
            .find_map(|child| child.downcast::<Button>().ok())
        else {
            continue;
        };
        if page == active_page {
            button.style_context().add_class("tab-active");
        } else {
            button.style_context().remove_class("tab-active");
        }
    }
}

fn close_tab(stack: &Stack, tab_bar: &GtkBox, window: Option<&ApplicationWindow>, view: &WebView) {
    let pages = stack.children();
    if pages.len() <= 1 {
        if let Some(window) = window {
            window.close();
        }
        return;
    }
    let Some(index) = pages
        .iter()
        .position(|page| *page == view.clone().upcast::<gtk::Widget>())
    else {
        return;
    };
    let tabs = tab_bar.children();
    stack.remove(view);
    if let Some(tab) = tabs.get(index) {
        tab_bar.remove(tab);
    }
    update_tab_selection(stack, tab_bar);
}

fn build_browser(app: &Application) {
    let window = ApplicationWindow::new(app);
    window.set_title("Ruster");
    window.set_default_size(1240, 820);
    let user_icon =
        dirs::data_local_dir().map(|base| base.join("icons/hicolor/scalable/apps/ruster.svg"));
    let icon_paths = [
        user_icon,
        Some(std::path::PathBuf::from(
            "/usr/share/icons/hicolor/scalable/apps/ruster.svg",
        )),
        Some(std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/ruster.svg"
        ))),
    ];
    for icon_path in icon_paths.into_iter().flatten() {
        if icon_path.is_file() && window.set_icon_from_file(&icon_path).is_ok() {
            break;
        }
    }

    let root = GtkBox::new(Orientation::Vertical, 0);
    let toolbar = GtkBox::new(Orientation::Horizontal, 4);
    toolbar.set_margin_top(5);
    toolbar.set_margin_bottom(5);
    toolbar.set_margin_start(8);
    toolbar.set_margin_end(8);
    toolbar.style_context().add_class("toolbar");

    let back = Button::with_label("‹");
    back.set_tooltip_text(Some("Back"));
    let forward = Button::with_label("›");
    forward.set_tooltip_text(Some("Forward"));
    back.set_sensitive(true);
    let reload = Button::with_label("↻");
    reload.set_tooltip_text(Some("Reload"));
    let address = Entry::new();
    address.set_hexpand(true);
    address.set_placeholder_text(Some("Search Google or enter an address"));
    address.style_context().add_class("address");
    let bookmark = Button::with_label("☆");
    bookmark.set_tooltip_text(Some("Add or remove bookmark"));
    let add_tab = Button::with_label("+");
    add_tab.set_tooltip_text(Some("New tab"));
    add_tab.style_context().add_class("browser-control");
    add_tab.style_context().add_class("tab-add");
    // HeaderBar stretches children to its full height; keep the button compact and centered.
    add_tab.set_valign(gtk::Align::Center);
    for button in [&back, &forward, &reload, &bookmark] {
        button.style_context().add_class("browser-control");
    }
    toolbar.pack_start(&back, false, false, 0);
    toolbar.pack_start(&forward, false, false, 0);
    toolbar.pack_start(&reload, false, false, 0);
    toolbar.pack_start(&address, true, true, 0);
    toolbar.pack_start(&bookmark, false, false, 0);

    let bookmark_bar = GtkBox::new(Orientation::Horizontal, 6);
    bookmark_bar.set_margin_start(14);
    bookmark_bar.set_margin_end(14);
    bookmark_bar.set_margin_top(4);
    bookmark_bar.set_margin_bottom(4);
    let tab_bar = GtkBox::new(Orientation::Horizontal, 5);
    tab_bar.set_margin_start(8);
    tab_bar.set_margin_end(0);
    tab_bar.set_margin_top(0);
    tab_bar.set_margin_bottom(0);
    tab_bar.style_context().add_class("tab-bar");
    tab_bar.set_hexpand(false);
    let header_bar = HeaderBar::new();
    header_bar.set_show_close_button(true);
    // The default centered window title overlaps custom tab controls in a HeaderBar.
    header_bar.set_custom_title(None::<&gtk::Widget>);
    header_bar.pack_start(&tab_bar);
    header_bar.pack_start(&add_tab);
    window.set_titlebar(Some(&header_bar));
    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let status_bar = GtkBox::new(Orientation::Horizontal, 8);
    status_bar.set_margin_start(14);
    status_bar.set_margin_end(14);
    status_bar.set_margin_top(5);
    status_bar.set_margin_bottom(5);
    let status = Label::new(Some("Ready"));
    status.style_context().add_class("status");
    status_bar.pack_start(&status, false, false, 0);
    root.pack_start(&toolbar, false, false, 0);
    root.pack_start(&bookmark_bar, false, false, 0);
    root.pack_start(
        &gtk::Separator::new(Orientation::Horizontal),
        false,
        false,
        0,
    );
    root.pack_start(&stack, true, true, 0);
    root.pack_start(
        &gtk::Separator::new(Orientation::Horizontal),
        false,
        false,
        0,
    );
    root.pack_start(&status_bar, false, false, 0);
    window.add(&root);

    let bookmarks = Rc::new(RefCell::new(load_bookmarks()));
    let browser = Browser {
        window: window.clone(),
        stack: stack.clone(),
        tab_bar: tab_bar.clone(),
        address: address.clone(),
        status: status.clone(),
        bookmarks: bookmarks.clone(),
        bookmark_bar: bookmark_bar.clone(),
        next_tab_id: Rc::new(Cell::new(0)),
    };
    render_bookmarks(&bookmark_bar, &bookmarks.borrow(), &address);
    browser.new_tab(None);

    let nav = browser.handles();
    address.connect_activate(move |entry| {
        if let Some(view) = nav.current_view() {
            view.load_uri(&make_url(entry.text().as_str()));
        }
    });
    let nav = browser.handles();
    back.connect_clicked(move |_| {
        if let Some(view) = nav.current_view() {
            if view.can_go_back() {
                view.go_back();
            } else if let Some(status) = nav.status.upgrade() {
                status.set_text("There is no earlier page in this tab yet");
            }
        }
    });
    let nav = browser.handles();
    forward.connect_clicked(move |_| {
        if let Some(view) = nav.current_view() {
            if view.can_go_forward() {
                view.go_forward();
            } else if let Some(status) = nav.status.upgrade() {
                status.set_text("There is no later page in this tab yet");
            }
        }
    });
    let nav = browser.handles();
    reload.connect_clicked(move |_| {
        if let Some(view) = nav.current_view() {
            if view.is_loading() {
                view.stop_loading();
            } else {
                view.reload();
            }
        }
    });
    let nav = browser.handles();
    add_tab.connect_clicked(move |_| nav.new_tab(None));
    let nav = browser.handles();
    bookmark.connect_clicked(move |_| nav.toggle_bookmark());
    let nav = browser.handles();
    stack.connect_visible_child_notify(move |stack| {
        let Some(view) = stack
            .visible_child()
            .and_then(|child| child.downcast::<WebView>().ok())
        else {
            return;
        };
        if let Some(address) = nav.address.upgrade() {
            let uri = view.uri().unwrap_or_default();
            address.set_text(if uri.as_str() == "about:blank" {
                ""
            } else {
                uri.as_str()
            });
        }
        let title = view
            .title()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "New tab".into());
        if let Some(window) = nav.window.upgrade() {
            window.set_title(&format!("{title} — Ruster"));
        }
        if let Some(status) = nav.status.upgrade() {
            let uri = view.uri().unwrap_or_default();
            status.set_text(if uri.starts_with("https://") {
                "Secure connection"
            } else if uri.as_str() == "about:blank" {
                "New tab — ready to search"
            } else {
                "Not secure"
            });
        }
        if let Some(tab_bar) = nav.tab_bar.upgrade() {
            update_tab_selection(stack, &tab_bar);
        }
    });

    let nav = browser.handles();
    window.connect_key_press_event(move |_, event| {
        use gtk::gdk::keys::constants as key;
        let control = event.state().contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let alt = event.state().contains(gtk::gdk::ModifierType::MOD1_MASK);
        let handled = if control && event.keyval() == key::l {
            if let Some(address) = nav.address.upgrade() {
                address.grab_focus();
                address.select_region(0, -1);
            }
            true
        } else if control && event.keyval() == key::t {
            nav.new_tab(None);
            true
        } else if control && event.keyval() == key::w {
            nav.close_current_tab();
            true
        } else if control && event.keyval() == key::r {
            if let Some(view) = nav.current_view() {
                view.reload();
            }
            true
        } else if alt && event.keyval() == key::Left {
            if let Some(view) = nav.current_view() {
                view.go_back();
            }
            true
        } else if alt && event.keyval() == key::Right {
            if let Some(view) = nav.current_view() {
                view.go_forward();
            }
            true
        } else {
            false
        };
        if handled {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    install_css();
    window.show_all();
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    let _ = provider.load_from_data(b"
        window { background-color: #f4f5f7; }
        .toolbar { background-color: #f4f5f7; }
        headerbar { color: #f4f5f7; background-color: #292d32; border: none; }
        headerbar .tab-bar { background-color: transparent; border: none; }
        .tab-bar { background-color: #e9ecf1; border-bottom: 1px solid #d7dce4; }
        button.tab-switch { color: #414a57; background-color: #e1e5eb; border: 1px solid #d1d6df; border-bottom: 0; border-radius: 8px 8px 0 0; min-height: 38px; padding: 0 9px; }
        button.tab-switch:hover { color: #202833; background-color: #f7f8fa; }
        button.tab-switch.tab-active { color: #1d2733; background-color: #ffffff; border-color: #cfd5df; }
        entry.address { color: #20242b; caret-color: #20242b; background-color: #ffffff; border: 1px solid #d6dae1; border-radius: 14px; min-height: 25px; padding: 3px 12px; font-size: 13px; }
        entry.address:focus { color: #20242b; background-color: #ffffff; border-color: #8da8c9; }
        button.browser-control { color: #303944; background-color: #ffffff; border: 1px solid #d9dde3; border-radius: 7px; min-width: 28px; min-height: 24px; padding: 0 5px; margin: 0; }
        button.browser-control:hover { color: #172335; background-color: #e8edf4; border-color: #c4ccd7; }
        button.browser-control:active { background-color: #dce4ee; }
        button.tab-add { min-width: 24px; min-height: 24px; padding: 0; margin-left: 4px; font-size: 14px; }
        button.tab-close { color: #6b7380; min-width: 20px; min-height: 20px; padding: 0; }
        button.tab-close:hover { color: #1f2937; background-color: #e4e8ef; }
        .status { color: #66717f; font-size: 11px; }
    ");
    if let Some(screen) = gtk::gdk::Screen::default() {
        gtk::StyleContext::add_provider_for_screen(
            &screen,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn make_url(input: &str) -> String {
    let input = input.trim();
    if input.starts_with("https://") || input.starts_with("http://") {
        return input.to_owned();
    }
    if input.contains(' ') || !input.contains('.') {
        return format!("https://www.google.com/search?q={}", encode_query(input));
    }
    format!("https://{input}")
}
fn encode_query(input: &str) -> String {
    input
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn bookmarks_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ruster")
        .join("bookmarks.json")
}
fn load_bookmarks() -> Vec<Bookmark> {
    fs::read(bookmarks_path())
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}
fn save_bookmarks(bookmarks: &[Bookmark]) -> std::io::Result<()> {
    let path = bookmarks_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(bookmarks).unwrap_or_default(),
    )
}
fn render_bookmarks(bar: &GtkBox, bookmarks: &[Bookmark], address: &Entry) {
    for child in bar.children() {
        bar.remove(&child);
    }
    for item in bookmarks {
        let button = Button::with_label(&item.title.chars().take(18).collect::<String>());
        button.style_context().add_class("flat");
        let url = item.url.clone();
        let address = address.clone();
        button.connect_clicked(move |_| {
            address.set_text(&url);
            address.emit_by_name::<()>("activate", &[]);
        });
        bar.pack_start(&button, false, false, 0);
    }
    bar.show_all();
}

fn main() {
    let app = Application::new(Some("io.ruster.Browser"), Default::default());
    app.connect_activate(build_browser);
    app.run();
}
