//! Serialized work queue: costly work never owns or mutates the live editor.
use crate::{
    format::DecodedPlan,
    model::{PlanFile, Snapshot},
};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};

pub(super) enum Outcome {
    Saved {
        path: PathBuf,
        snapshot: Snapshot,
        result: Result<(), String>,
    },
    Opened {
        path: PathBuf,
        result: Result<(DecodedPlan, bool), String>,
    },
    Exported {
        path: PathBuf,
        format: String,
        result: Result<(), String>,
    },
    Rooms {
        plan: PlanFile,
        limited: bool,
    },
    Recovery(Result<(), String>),
    Failed {
        recovery: bool,
    },
}

type Task = Box<dyn FnOnce() -> Outcome + Send>;
#[derive(Default)]
struct Queue {
    tasks: VecDeque<(bool, Task)>,
    stop: bool,
}

pub(super) struct Worker {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    results: mpsc::Receiver<Outcome>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn new(context: eframe::egui::Context) -> Self {
        let queue = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let (sender, results) = mpsc::channel();
        let shared = Arc::clone(&queue);
        let thread = thread::spawn(move || {
            loop {
                let task = {
                    let (mutex, wake) = &*shared;
                    let mut queue = mutex.lock().expect("worker queue");
                    while queue.tasks.is_empty() && !queue.stop {
                        queue = wake.wait(queue).expect("worker queue");
                    }
                    match queue.tasks.pop_front() {
                        Some(task) => task,
                        None => break,
                    }
                };
                let (recovery, task) = task;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task))
                    .unwrap_or(Outcome::Failed { recovery });
                if sender.send(result).is_err() {
                    break;
                }
                context.request_repaint();
            }
        });
        Self {
            queue,
            results,
            thread: Some(thread),
        }
    }

    pub fn submit(&self, recovery: bool, task: Task) {
        let (mutex, wake) = &*self.queue;
        let mut queue = mutex.lock().expect("worker queue");
        // Retain at most one pending recovery, replacing it with the newest state.
        // Foreground operations stay ordered relative to all remaining work.
        if recovery {
            queue.tasks.retain(|(is_recovery, _)| !is_recovery);
        }
        queue.tasks.push_back((recovery, task));
        wake.notify_one();
    }

    pub fn poll(&self) -> Option<Outcome> {
        self.results.try_recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (mutex, wake) = &*self.queue;
        mutex.lock().expect("worker queue").stop = true;
        wake.notify_one();
        // Flush the latest recovery before process exit, including queued work.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Outcome, Worker};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    use std::time::Duration;

    #[test]
    fn worker_coalesces_recovery_and_flushes_before_exit() {
        let worker = Worker::new(eframe::egui::Context::default());
        let (started, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        worker.submit(
            false,
            Box::new(move || {
                started.send(()).expect("start");
                blocked
                    .recv_timeout(Duration::from_secs(5))
                    .expect("release");
                Outcome::Recovery(Ok(()))
            }),
        );
        ready
            .recv_timeout(Duration::from_secs(5))
            .expect("worker starts off-thread");
        let state = Arc::new(AtomicUsize::new(0));
        for value in [1, 2] {
            let state = Arc::clone(&state);
            worker.submit(
                true,
                Box::new(move || {
                    state.store(value, Ordering::SeqCst);
                    Outcome::Recovery(Ok(()))
                }),
            );
        }
        assert_eq!(worker.queue.0.lock().expect("queue").tasks.len(), 1);
        release.send(()).expect("release");
        drop(worker);
        assert_eq!(state.load(Ordering::SeqCst), 2);
    }
}
