//! The browser side of [`plnwrk_ui::Host`]: a file input to open, a download to save, the
//! question before leaving the page, and the browser's local storage for recovery copies.
//! Nothing leaves the computer: files go through the browser's own dialogs only.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use eframe::egui;
use plnwrk_ui::{Host, Upload};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::{Closure, JsValue};

/// A JavaScript error as text.
fn reason(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

fn document() -> Result<web_sys::Document, String> {
    web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| "no document".to_owned())
}

fn local_storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or_else(|| "no window".to_owned())?
        .local_storage()
        .map_err(|e| reason(&e))?
        .ok_or_else(|| "no local storage".to_owned())
}

/// Every key and value of the browser's local storage.
fn all_stored() -> Vec<(String, String)> {
    let Ok(storage) = local_storage() else {
        return Vec::new();
    };
    let count = storage.length().unwrap_or(0);
    (0..count)
        .filter_map(|i| storage.key(i).ok().flatten())
        .filter_map(|key| {
            let value = storage.get_item(&key).ok().flatten()?;
            Some((key, value))
        })
        .collect()
}

/// Session storage key of this tab's id.
const TAB_KEY: &str = "plnwrk.tab";
/// Name of the browser lock an open tab holds: this, then its id.
const LOCK: &str = "plnwrk.tab.";

/// The tab's id from its own session storage (survives a reload, copied with a duplicated
/// tab), or a new one.
fn stored_tab_id() -> String {
    let storage = web_sys::window().and_then(|w| w.session_storage().ok().flatten());
    if let Some(id) = storage
        .as_ref()
        .and_then(|s| s.get_item(TAB_KEY).ok().flatten())
    {
        return id;
    }
    new_tab_id()
}

/// A new id for this tab, kept in its session storage.
fn new_tab_id() -> String {
    let id = plnwrk_platform::instance();
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.set_item(TAB_KEY, &id);
    }
    id
}

/// The browser's lock manager (Web Locks API), if it has one. Reached through JavaScript
/// directly: web-sys has it only behind an unstable flag.
fn lock_manager() -> Option<JsValue> {
    let navigator = web_sys::window()?.navigator();
    let locks = js_sys::Reflect::get(&navigator, &JsValue::from_str("locks")).ok()?;
    (!locks.is_undefined() && !locks.is_null()).then_some(locks)
}

/// Asks for the lock `name` and holds it as long as the page lives. `if_available`: do not
/// wait for it. `got` learns whether this page holds it.
fn hold_lock(locks: &JsValue, name: &str, if_available: bool, got: impl FnOnce(bool) + 'static) {
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &options,
        &JsValue::from_str("ifAvailable"),
        &JsValue::from_bool(if_available),
    );
    let callback = Closure::once_into_js(move |lock: JsValue| -> js_sys::Promise {
        let held = !lock.is_null() && !lock.is_undefined();
        got(held);
        if held {
            // Never settles: the lock is released only when the page ends.
            js_sys::Promise::new(&mut |_, _| {})
        } else {
            js_sys::Promise::resolve(&JsValue::UNDEFINED)
        }
    });
    let request = js_sys::Reflect::get(locks, &JsValue::from_str("request"))
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    if let Some(request) = request {
        let _ = request.call3(locks, &JsValue::from_str(name), &options, &callback);
    }
}

/// The ids of the tabs that hold their lock now.
async fn open_tabs(locks: &JsValue) -> Option<Vec<String>> {
    let query = js_sys::Reflect::get(locks, &JsValue::from_str("query"))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let promise = query
        .call0(locks)
        .ok()?
        .dyn_into::<js_sys::Promise>()
        .ok()?;
    let snapshot = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
    let held = js_sys::Reflect::get(&snapshot, &JsValue::from_str("held")).ok()?;
    let held: js_sys::Array = held.dyn_into().ok()?;
    let mut tabs: Vec<String> = held
        .iter()
        .filter_map(|lock| js_sys::Reflect::get(&lock, &JsValue::from_str("name")).ok())
        .filter_map(|name| name.as_string())
        .filter_map(|name| name.strip_prefix(LOCK).map(str::to_owned))
        .collect();
    tabs.sort();
    Some(tabs)
}

/// The browser around the app.
pub struct WebHost {
    /// To wake the app up when a chosen file has been read.
    ctx: egui::Context,
    /// The file chosen last (or why it could not be read), until the app takes it.
    upload: Rc<RefCell<Option<Result<Upload, String>>>>,
    /// The id of this tab (a duplicated tab gets a new one once the lock shows it).
    tab: Rc<RefCell<String>>,
    /// The tabs open now (`None` until the browser told), and whether that changed.
    live: Rc<RefCell<(Option<Vec<String>>, bool)>>,
    /// Whether leaving the page asks first.
    leave: Rc<Cell<bool>>,
}

impl WebHost {
    /// Sets up the question before leaving the page (asked only while there is unsaved work).
    pub fn new(ctx: egui::Context) -> Self {
        let leave = Rc::new(Cell::new(false));
        let ask = Rc::clone(&leave);
        let on_leave = Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(
            move |event: web_sys::BeforeUnloadEvent| {
                if ask.get() {
                    event.prevent_default();
                    event.set_return_value("");
                }
            },
        );
        if let Some(window) = web_sys::window()
            && let Err(error) = window
                .add_event_listener_with_callback("beforeunload", on_leave.as_ref().unchecked_ref())
        {
            log::warn!("plnwrk: no question before leaving: {}", reason(&error));
        }
        // Lives as long as the page.
        on_leave.forget();
        let host = Self {
            ctx,
            upload: Rc::new(RefCell::new(None)),
            tab: Rc::new(RefCell::new(stored_tab_id())),
            live: Rc::new(RefCell::new((None, false))),
            leave,
        };
        host.watch_tabs();
        host
    }

    /// Holds this tab's lock (taking a new id if another open tab holds it: a duplicate) and
    /// asks the browser every few seconds which tabs are open. Without Web Locks every other
    /// tab counts as closed.
    fn watch_tabs(&self) {
        let Some(locks) = lock_manager() else {
            *self.live.borrow_mut() = (Some(Vec::new()), true);
            return;
        };
        let tab = Rc::clone(&self.tab);
        let changed = Rc::clone(&self.live);
        let again = locks.clone();
        let id = tab.borrow().clone();
        hold_lock(&locks, &format!("{LOCK}{id}"), true, move |held| {
            if !held {
                // Another open tab has this id: this one is a duplicate.
                let id = new_tab_id();
                hold_lock(&again, &format!("{LOCK}{id}"), false, |_| {});
                *tab.borrow_mut() = id;
                changed.borrow_mut().1 = true;
            }
        });
        let live = Rc::clone(&self.live);
        let ctx = self.ctx.clone();
        let ask = Closure::<dyn FnMut()>::new(move || {
            let live = Rc::clone(&live);
            let ctx = ctx.clone();
            let locks = locks.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let Some(tabs) = open_tabs(&locks).await else {
                    return;
                };
                let mut known = live.borrow_mut();
                if known.0.as_ref() != Some(&tabs) {
                    *known = (Some(tabs), true);
                    ctx.request_repaint();
                }
            });
        });
        if let Some(window) = web_sys::window() {
            // Right away, then every two seconds.
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                ask.as_ref().unchecked_ref(),
                0,
            );
            let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
                ask.as_ref().unchecked_ref(),
                2_000,
            );
        }
        // Lives as long as the page.
        ask.forget();
    }

    fn ask_for_file(&self) -> Result<(), String> {
        let input: web_sys::HtmlInputElement = document()?
            .create_element("input")
            .map_err(|e| reason(&e))?
            .dyn_into()
            .map_err(|_| "no file input".to_owned())?;
        input.set_type("file");
        input.set_accept(".plnwrk");
        let chosen = input.clone();
        let slot = Rc::clone(&self.upload);
        let ctx = self.ctx.clone();
        let on_change = Closure::once_into_js(move || {
            let Some(file) = chosen.files().and_then(|files| files.get(0)) else {
                return;
            };
            let name = file.name();
            wasm_bindgen_futures::spawn_local(async move {
                let read = wasm_bindgen_futures::JsFuture::from(file.array_buffer())
                    .await
                    .map(|buffer| Upload {
                        name,
                        bytes: js_sys::Uint8Array::new(&buffer).to_vec(),
                    })
                    .map_err(|error| reason(&error));
                *slot.borrow_mut() = Some(read);
                ctx.request_repaint();
            });
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
        Ok(())
    }

    fn offer_download(name: &str, bytes: &[u8]) -> Result<(), String> {
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
            .map_err(|e| reason(&e))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| reason(&e))?;
        let link: web_sys::HtmlAnchorElement = document()?
            .create_element("a")
            .map_err(|e| reason(&e))?
            .dyn_into()
            .map_err(|_| "no link".to_owned())?;
        link.set_href(&url);
        link.set_download(name);
        link.click();
        // Freed a little later: some browsers start the download only after the click returns.
        let free = Closure::once_into_js(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                free.unchecked_ref(),
                60_000,
            );
        }
        Ok(())
    }
}

impl Host for WebHost {
    fn pick_open(&mut self) -> Option<std::path::PathBuf> {
        None
    }

    fn pick_save(&mut self, _suggested_name: &str) -> Option<std::path::PathBuf> {
        None
    }

    fn browser(&self) -> bool {
        true
    }

    fn request_upload(&mut self) {
        if let Err(error) = self.ask_for_file() {
            log::warn!("plnwrk: no file dialog: {error}");
        }
    }

    fn uploaded(&mut self) -> Option<Result<Upload, String>> {
        self.upload.borrow_mut().take()
    }

    fn tab(&mut self) -> String {
        self.tab.borrow().clone()
    }

    fn download(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        Self::offer_download(name, bytes)
    }

    fn set_leave_warning(&mut self, on: bool) {
        self.leave.set(on);
    }

    fn store(&mut self, key: &str, value: Option<&str>) -> Result<(), String> {
        let storage = local_storage()?;
        match value {
            Some(value) => storage.set_item(key, value),
            None => storage.remove_item(key),
        }
        .map_err(|e| reason(&e))
    }

    fn stored(&mut self) -> Vec<(String, String)> {
        all_stored()
    }

    fn live_tabs(&mut self) -> Option<Vec<String>> {
        self.live.borrow().0.clone()
    }

    fn live_changed(&mut self) -> bool {
        std::mem::take(&mut self.live.borrow_mut().1)
    }
}
