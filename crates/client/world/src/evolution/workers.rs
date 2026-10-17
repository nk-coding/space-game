//! The web's executor (GDR-0092 step 9, ADR-0043): the lab's jobs in web workers, each running the `lab-worker`
//! binary (`crates/client/lab-worker`: `arena`, `data`, `sim` and wasm-bindgen, no Bevy), so a run computes beside the
//! frame and not in it. `navigator.hardwareConcurrency` less one of them (the frame's core), started the first time a
//! run computes. A worker says it is ready with the content it fights with (`arena::worker::Reply::Ready`); one whose
//! content differs from the game's is let go, so results of two contents never mix. Chunks wait in a queue for a
//! free worker; a worker that fails hands its chunk back to the queue and is let go. With no worker left (none would
//! load, or `?lab_workers=0`) the run computes in the frame's slices, as before. No shared memory: plain messages,
//! so no COOP/COEP headers are needed and it works over plain `http://`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use arena::evolve::{Job, JobResult};
use arena::worker::{Reply, Task};
use bevy::log::{info, warn};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use super::out::Executor;

/// The worker's script: Trunk's loader for the `lab-worker` binary (`index.html`), next to the game's files.
const SCRIPT: &str = "./lab-worker_loader.js";

/// What a worker is doing.
enum State {
    Loading,
    Ready,
    /// Running a chunk: its batch, first job and message (sent again elsewhere if the worker fails).
    Busy(u64, usize, String),
    Gone,
}

/// What came from a worker, by its index.
enum Heard {
    Message(String),
    Failed(String),
}

struct Slot {
    worker: web_sys::Worker,
    state: State,
    // Kept alive while the worker is.
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
}

/// The workers (a non-send resource: browser objects stay on the main thread).
pub struct Workers {
    slots: Vec<Slot>,
    inbox: Rc<RefCell<Vec<(usize, Heard)>>>,
    /// The game's content as the workers must fight with it (`arena::worker::content_key`).
    key: u64,
    queue: VecDeque<(u64, usize, String)>,
    done: Vec<(u64, usize, Vec<JobResult>)>,
}

impl Workers {
    /// Starts `navigator.hardwareConcurrency − 1` workers (at least one) for content `key`; none when the browser
    /// can't.
    pub fn start(key: u64) -> Workers {
        let inbox: Rc<RefCell<Vec<(usize, Heard)>>> = Rc::default();
        let n = web_sys::window().map_or(1.0, |w| w.navigator().hardware_concurrency()).max(2.0) as usize - 1;
        let mut slots = Vec::new();
        for k in 0..n {
            let worker = match web_sys::Worker::new(SCRIPT) {
                Ok(w) => w,
                Err(e) => {
                    warn!("lab: no web worker ({e:?}): the run computes in the frame");
                    break;
                }
            };
            let heard = inbox.clone();
            let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                let text = e.data().as_string().unwrap_or_default();
                heard.borrow_mut().push((k, Heard::Message(text)));
            });
            let heard = inbox.clone();
            let on_error = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |e: web_sys::ErrorEvent| {
                heard.borrow_mut().push((k, Heard::Failed(e.message())));
            });
            worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
            worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            slots.push(Slot { worker, state: State::Loading, _on_message: on_message, _on_error: on_error });
        }
        info!("lab: {} web workers starting", slots.len());
        Workers { slots, inbox, key, queue: VecDeque::new(), done: Vec::new() }
    }

    /// No worker started, or every one is gone: the run computes in the frame.
    pub fn gone(&self) -> bool {
        self.slots.iter().all(|s| matches!(s.state, State::Gone))
    }

    /// Lets worker `k` go; its chunk, if it had one, goes back to the queue.
    fn drop_worker(&mut self, k: usize, why: &str) {
        let slot = &mut self.slots[k];
        warn!("lab: web worker {k} let go: {why}");
        slot.worker.terminate();
        if let State::Busy(batch, first, text) = std::mem::replace(&mut slot.state, State::Gone) {
            self.queue.push_front((batch, first, text));
        }
    }

    /// Takes what the workers said and gives the free ones the waiting chunks.
    fn poll(&mut self) {
        let heard = std::mem::take(&mut *self.inbox.borrow_mut());
        for (k, h) in heard {
            if matches!(self.slots[k].state, State::Gone) {
                continue;
            }
            match h {
                Heard::Failed(why) => self.drop_worker(k, &why),
                Heard::Message(text) => match arena::worker::read::<Reply>(&text) {
                    Ok(Reply::Ready { content }) if content == self.key => self.slots[k].state = State::Ready,
                    Ok(Reply::Ready { content }) => self.drop_worker(k, &format!("it fights with other content ({content:016x})")),
                    Ok(Reply::Done { batch, first, results }) => {
                        self.slots[k].state = State::Ready;
                        self.done.push((batch, first as usize, results));
                    }
                    Ok(Reply::Refused(why)) => self.drop_worker(k, &why),
                    Err(e) => self.drop_worker(k, &format!("its message doesn't read: {e}")),
                },
            }
        }
        for k in 0..self.slots.len() {
            if !matches!(self.slots[k].state, State::Ready) {
                continue;
            }
            let Some((batch, first, text)) = self.queue.pop_front() else { break };
            match self.slots[k].worker.post_message(&text.as_str().into()) {
                Ok(()) => self.slots[k].state = State::Busy(batch, first, text),
                Err(e) => {
                    self.queue.push_front((batch, first, text));
                    self.drop_worker(k, &format!("{e:?}"));
                }
            }
        }
    }
}

impl Executor for Workers {
    fn width(&self) -> usize {
        self.slots.iter().filter(|s| matches!(s.state, State::Ready | State::Busy(..))).count()
    }

    fn spawn(&mut self, batch: u64, first: usize, jobs: &[Job]) {
        let task = Task { batch, first: first as u32, content: self.key, jobs: jobs.to_vec() };
        self.queue.push_back((batch, first, arena::worker::write(&task)));
        self.poll();
    }

    fn finished(&mut self) -> Vec<(u64, usize, Vec<JobResult>)> {
        self.poll();
        std::mem::take(&mut self.done)
    }
}
