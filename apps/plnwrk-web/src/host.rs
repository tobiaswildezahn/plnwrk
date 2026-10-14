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

/// The id of this tab: kept in the tab's own session storage, so it survives a reload (and
/// the tab finds its own recovery copies), but every other tab has its own – also a
/// duplicated tab, which inherits the session storage (see `plnwrk_ui::keep_tab_id_now`).
fn tab_id() -> String {
    const KEY: &str = "plnwrk.tab";
    let storage = web_sys::window().and_then(|w| w.session_storage().ok().flatten());
    if let Some(id) = storage
        .as_ref()
        .and_then(|s| s.get_item(KEY).ok().flatten())
        && plnwrk_ui::keep_tab_id_now(&all_stored(), &id)
    {
        return id;
    }
    let id = plnwrk_platform::instance();
    if let Some(storage) = storage {
        let _ = storage.set_item(KEY, &id);
    }
    id
}

/// The browser around the app.
pub struct WebHost {
    /// To wake the app up when a chosen file has been read.
    ctx: egui::Context,
    /// The file chosen last (or why it could not be read), until the app takes it.
    upload: Rc<RefCell<Option<Result<Upload, String>>>>,
    /// The id of this tab.
    tab: String,
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
        let tab = tab_id();
        Self::watch_page(&tab);
        Self {
            ctx,
            upload: Rc::new(RefCell::new(None)),
            tab,
            leave,
        }
    }

    /// Keeps this tab's sign of life right when the page changes: gone when the page goes
    /// (reload, close), marked "hidden" when the tab goes to the background (where the browser
    /// slows it down and it cannot say it is open).
    fn watch_page(tab: &str) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let key = format!("{}{tab}", plnwrk_ui::LIVE);
        let gone_key = key.clone();
        // The page going hides it too ("visibilitychange" comes after "pagehide"): it must not
        // write its sign of life back then. A page restored from the back/forward cache lives
        // again.
        let leaving = Rc::new(Cell::new(false));
        let gone = Rc::clone(&leaving);
        let on_gone = Closure::<dyn FnMut()>::new(move || {
            gone.set(true);
            if let Ok(storage) = local_storage() {
                let _ = storage.remove_item(&gone_key);
            }
        });
        let back = Rc::clone(&leaving);
        let on_back = Closure::<dyn FnMut()>::new(move || back.set(false));
        let on_hidden = Closure::<dyn FnMut()>::new(move || {
            let hidden = document().is_ok_and(|d| d.hidden());
            if hidden
                && !leaving.get()
                && let Ok(storage) = local_storage()
            {
                let _ = storage.set_item(&key, &plnwrk_ui::live_now(true));
            }
        });
        let _ =
            window.add_event_listener_with_callback("pagehide", on_gone.as_ref().unchecked_ref());
        let _ =
            window.add_event_listener_with_callback("pageshow", on_back.as_ref().unchecked_ref());
        if let Ok(document) = document() {
            let _ = document.add_event_listener_with_callback(
                "visibilitychange",
                on_hidden.as_ref().unchecked_ref(),
            );
        }
        // Live as long as the page.
        on_gone.forget();
        on_back.forget();
        on_hidden.forget();
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
        self.tab.clone()
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

    fn hidden(&self) -> bool {
        document().is_ok_and(|d| d.hidden())
    }
}
