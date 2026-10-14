//! plnwrk in the browser: finds the canvas of `index.html` and hands it to `plnwrk_ui`.
//!
//! Build and serve with `trunk serve apps/plnwrk-web/index.html`.

/// Id of the canvas element in `index.html`.
#[cfg(target_arch = "wasm32")]
const CANVAS_ID: &str = "plnwrk_canvas";

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Logging may already be set up (e.g. on hot reload); that is fine.
    let _ = eframe::WebLogger::init(log::LevelFilter::Info);

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(CANVAS_ID))
            .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok());
        let Some(canvas) = canvas else {
            log::error!("plnwrk: no <canvas id=\"{CANVAS_ID}\"> in the page");
            return;
        };
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(plnwrk_ui::PlnwrkApp::new(cc)))),
            )
            .await;
        if let Err(error) = started {
            log::error!("plnwrk could not start: {error:?}");
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("plnwrk-web runs in the browser: trunk serve apps/plnwrk-web/index.html");
}
