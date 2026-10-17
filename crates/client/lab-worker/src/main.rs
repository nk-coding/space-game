//! The evolution lab's web worker (GDR-0092 step 9, ADR-0043). The web build runs it beside the game
//! (`navigator.hardwareConcurrency` less one of them, `world::evolution`'s workers): it loads the base content, says
//! it is ready with the content it fights with, then answers each task (a few of a generation's jobs, as RON text)
//! with their results (`arena::worker::answer`). It only fights: the game breeds, closes generations and writes the
//! storage. Trunk builds it with the game (`crates/client/game/index.html`, `data-type="worker"`) and writes its
//! loader, `lab-worker_loader.js`, next to the game's files.

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let content = data::base_content();
    let key = arena::worker::content_key(&content);
    let reply_to = scope.clone();
    let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
        let text = e.data().as_string().unwrap_or_default();
        let reply = arena::worker::answer(&content, key, &text);
        // Posting to its own page fails only when the page is gone, and then nobody waits for the answer.
        let _ = reply_to.post_message(&arena::worker::write(&reply).into());
    });
    scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // The handler lives as long as the worker does.
    on_message.forget();
    let _ = scope.post_message(&arena::worker::write(&arena::worker::Reply::Ready { content: key }).into());
}

/// Natively there is no worker to be: the desktop and Android builds run the lab's jobs on Bevy's compute threads.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("lab-worker is the evolution lab's web worker: the web build runs it in the browser (ADR-0043)");
}
