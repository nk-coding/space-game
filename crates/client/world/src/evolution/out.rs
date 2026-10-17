//! Jobs out of the frame (GDR-0092 "The executor", ADR-0043): a generation's jobs (or a validation's, or a rating's)
//! go out a few at a time to an [`Executor`] and come back in any order; [`Out`] keeps them in job order, so the run
//! gets them as `arena::Run::jobs` gave them and closes the generation as the frame's own slices would (the same run).
//!
//! * Natively the executor is Bevy's compute threads ([`Threads`]): as many tasks at once as cores less one.
//! * On the web it is the lab's web workers (`workers`, the `lab-worker` binary): `navigator.hardwareConcurrency`
//!   less one, each its own wasm instance; jobs and results go as RON text (`arena::worker`). Only the main thread
//!   breeds, closes generations and writes the storage; a worker only fights.

use arena::evolve::{Job, JobResult};

/// Jobs a chunk carries (a task natively, a message to a worker): about 5–20 ms natively, so flight gets the cores
/// back soon after it starts, and few enough that writing a chunk's message costs a frame little.
pub const CHUNK: usize = 4;

/// Where chunks of jobs run.
pub trait Executor {
    /// How many chunks it runs at once (0: nothing to run them on: no worker ready, or every one gone).
    fn width(&self) -> usize;
    /// Starts jobs `first..first + jobs.len()` of batch `batch`.
    fn spawn(&mut self, batch: u64, first: usize, jobs: &[Job]);
    /// The chunks that ended since the last call: their batch, their first job and the results in order.
    fn finished(&mut self) -> Vec<(u64, usize, Vec<JobResult>)>;
}

/// One batch's jobs out on an executor.
pub struct Out {
    /// The run and generation they are of (`u32::MAX`: a validation's or a rating's, in the run's place).
    pub id: String,
    pub generation: u32,
    /// Its number: results of another batch (one given up while its chunks ran) are dropped.
    batch: u64,
    jobs: Vec<Job>,
    results: Vec<Option<JobResult>>,
    /// The first job not handed out yet, and the chunks underway.
    next: usize,
    flying: usize,
}

impl Out {
    pub fn new(id: &str, generation: u32, batch: u64, jobs: Vec<Job>) -> Out {
        let n = jobs.len();
        Out { id: id.to_string(), generation, batch, jobs, results: vec![None; n], next: 0, flying: 0 }
    }

    /// Takes the chunks that ended and, unless `hold`, hands out more while the executor has room; returns the ticks
    /// the jobs that came in ran.
    pub fn pump(&mut self, exec: &mut dyn Executor, hold: bool) -> u64 {
        let mut ticks = 0;
        for (batch, first, rs) in exec.finished() {
            if batch != self.batch {
                continue;
            }
            self.flying -= 1;
            for (j, r) in rs.into_iter().enumerate() {
                ticks += r.ticks;
                self.results[first + j] = Some(r);
            }
        }
        while !hold && self.flying < exec.width() && self.next < self.jobs.len() {
            let end = (self.next + CHUNK).min(self.jobs.len());
            exec.spawn(self.batch, self.next, &self.jobs[self.next..end]);
            self.next = end;
            self.flying += 1;
        }
        ticks
    }

    pub fn done(&self) -> usize {
        self.results.iter().filter(|r| r.is_some()).count()
    }

    pub fn total(&self) -> usize {
        self.results.len()
    }

    /// Every job's result is in.
    pub fn finished(&self) -> bool {
        self.flying == 0 && self.results.iter().all(Option::is_some)
    }

    /// The results in job order (all of them: [`Out::finished`]).
    pub fn take(self) -> Vec<JobResult> {
        self.results.into_iter().map(|r| r.expect("every job is in")).collect()
    }
}

/// The native executor: Bevy's compute threads (`AsyncComputeTaskPool`), one task per chunk, as many at once as the
/// machine has cores less one (the frame's).
#[cfg(not(target_arch = "wasm32"))]
pub struct Threads {
    content: std::sync::Arc<data::Content>,
    width: usize,
    tasks: Vec<(u64, usize, bevy::tasks::Task<Vec<JobResult>>)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Threads {
    pub fn new(content: &data::Content) -> Threads {
        let width = std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(1).max(1));
        Threads { content: std::sync::Arc::new(content.clone()), width, tasks: Vec::new() }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Executor for Threads {
    fn width(&self) -> usize {
        self.width
    }

    fn spawn(&mut self, batch: u64, first: usize, jobs: &[Job]) {
        let (jobs, content) = (jobs.to_vec(), self.content.clone());
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { jobs.iter().map(|j| j.run(&content)).collect() });
        self.tasks.push((batch, first, task));
    }

    fn finished(&mut self) -> Vec<(u64, usize, Vec<JobResult>)> {
        use bevy::tasks::{block_on, futures_lite::future};
        let mut out = Vec::new();
        let mut k = 0;
        while k < self.tasks.len() {
            match block_on(future::poll_once(&mut self.tasks[k].2)) {
                Some(rs) => {
                    let (batch, first, _) = self.tasks.swap_remove(k);
                    out.push((batch, first, rs));
                }
                None => k += 1,
            }
        }
        out
    }
}
