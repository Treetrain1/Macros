use std::thread::JoinHandle;

pub(crate) struct ThreadPool {
    pub(crate) workers: Vec<JoinHandle<()>>,
}

impl ThreadPool {
    pub(crate) fn new() -> Self {
        ThreadPool { workers: Vec::new() }
    }

    pub(crate) fn add_worker(&mut self, worker: JoinHandle<()>) {
        self.workers.push(worker);
    }

    pub(crate) fn cleanup_completed_threads(&mut self) {
        self.workers.retain(|worker| !worker.is_finished());
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        for worker in self.workers.drain(..) {
            worker.join().expect("Failed to join worker thread");
        }
    }
}
