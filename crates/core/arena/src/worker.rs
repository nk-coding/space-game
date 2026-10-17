//! Jobs as messages, for the lab's web workers (GDR-0092 step 9, ADR-0043): the game hands a generation's jobs (or a
//! validation's, or a rating's) to workers as RON text and takes their results back the same way; a worker answers
//! with [`answer`]. A job's result depends only on the job and the content (`eval.rs`), and RON writes floats exactly,
//! so a result that went through a worker is the result the frame's own slices give. A worker fights with the content
//! it was built with: it says so first ([`Reply::Ready`] with its [`content_key`]) and refuses a task for other
//! content, so the game never mixes results of two contents.

use data::Content;
use serde::{Deserialize, Serialize};

use crate::evolve::{Job, JobResult};

/// What the game sends a worker: jobs `first..first + jobs.len()` of batch `batch`, for the content of `content`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub batch: u64,
    pub first: u32,
    pub content: u64,
    pub jobs: Vec<Job>,
}

/// What a worker sends back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Reply {
    /// Loaded and listening, with the content it fights with.
    Ready { content: u64 },
    /// The results of a task's jobs, in its order.
    Done { batch: u64, first: u32, results: Vec<JobResult> },
    /// A message it can't take (it doesn't read, or it is for other content), in words.
    Refused(String),
}

/// What a fight reads of the content, the lab's stand-in pilots included: [`Content::fingerprint`] and the lab's RON.
pub fn content_key(content: &Content) -> u64 {
    let lab = data::ron_options().to_string(&content.lab).expect("the lab serialises");
    data::hash::fnv1a64(format!("{:016x}{lab}", content.fingerprint()).as_bytes())
}

/// A message as text (RON).
pub fn write<T: Serialize>(message: &T) -> String {
    data::ron_options().to_string(message).expect("a message serialises")
}

/// Reads a message.
pub fn read<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    data::ron_options().from_str(text).map_err(|e| e.to_string())
}

/// A worker's answer to a message: the task's jobs run in order ([`Reply::Done`]), or why not. `key` is the worker's
/// [`content_key`].
pub fn answer(content: &Content, key: u64, text: &str) -> Reply {
    match read::<Task>(text) {
        Ok(t) if t.content != key => Reply::Refused(format!("a task for other content ({:016x}; this worker's is {key:016x})", t.content)),
        Ok(t) => Reply::Done { batch: t.batch, first: t.first, results: t.jobs.iter().map(|j| j.run(content)).collect() },
        Err(e) => Reply::Refused(format!("the task doesn't read: {e}")),
    }
}
