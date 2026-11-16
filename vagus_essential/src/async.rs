use std::any::Any;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

const PENDING: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETED: u8 = 2;
const FAILED: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskStatus {
    Pending = PENDING,
    Running = RUNNING,
    Completed = COMPLETED,
    Failed = FAILED,
}

impl TaskStatus {
    fn from_raw(value: u8) -> Self {
        match value {
            RUNNING => Self::Running,
            COMPLETED => Self::Completed,
            FAILED => Self::Failed,
            _ => Self::Pending,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct Shared<T> {
    result: Mutex<Option<T>>,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
    waker: Mutex<Option<Waker>>,
    status: AtomicU8,
}

impl<T> Shared<T> {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            panic: Mutex::new(None),
            waker: Mutex::new(None),
            status: AtomicU8::new(PENDING),
        }
    }

    fn wake_joiner(&self) {
        if let Some(waker) = lock(&self.waker).take() {
            waker.wake();
        }
    }
}

pub struct JoinHandle<T> {
    shared: Arc<Shared<T>>,
}

impl<T> JoinHandle<T> {
    pub fn status(&self) -> TaskStatus {
        TaskStatus::from_raw(self.shared.status.load(Ordering::Acquire))
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let shared = &self.get_mut().shared;
        if let Some(result) = lock(&shared.result).take() {
            return Poll::Ready(result);
        }
        if let Some(payload) = lock(&shared.panic).take() {
            resume_unwind(payload);
        }

        *lock(&shared.waker) = Some(cx.waker().clone());

        if let Some(result) = lock(&shared.result).take() {
            Poll::Ready(result)
        } else if let Some(payload) = lock(&shared.panic).take() {
            resume_unwind(payload)
        } else {
            Poll::Pending
        }
    }
}

/// A thread-per-task executor for futures that are safe to move between threads.
#[derive(Debug, Default, Clone, Copy)]
pub struct Worker;

impl Worker {
    pub const fn new() -> Self {
        Self
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let shared = Arc::new(Shared::new());
        let task_shared = Arc::clone(&shared);
        thread::spawn(move || {
            task_shared.status.store(RUNNING, Ordering::Release);
            match catch_unwind(AssertUnwindSafe(|| block_on(future))) {
                Ok(result) => {
                    let mut result_slot = lock(&task_shared.result);
                    *result_slot = Some(result);
                    task_shared.status.store(COMPLETED, Ordering::Release);
                }
                Err(payload) => {
                    let mut panic_slot = lock(&task_shared.panic);
                    *panic_slot = Some(payload);
                    task_shared.status.store(FAILED, Ordering::Release);
                }
            }
            task_shared.wake_joiner();
        });
        JoinHandle { shared }
    }
}

/// Queue-free alias for the thread-per-task worker.
pub type Thread = Worker;

/// Common spawn interface for executors that run `Send` futures.
pub trait AsyncExecutor {
    fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
}

impl AsyncExecutor for Worker {
    fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        Worker::spawn(self, future)
    }
}

/// A queue-free local worker that drives one future on the current thread.
pub struct State<T> {
    future: Option<Pin<Box<dyn Future<Output = T> + 'static>>>,
    result: Option<T>,
    status: TaskStatus,
}

impl<T> State<T> {
    pub fn new<F>(future: F) -> Self
    where
        F: Future<Output = T> + 'static,
    {
        Self {
            future: Some(Box::pin(future)),
            result: None,
            status: TaskStatus::Pending,
        }
    }

    pub fn status(&self) -> TaskStatus {
        self.status
    }

    pub fn result(&self) -> Option<&T> {
        self.result.as_ref()
    }

    pub fn take_result(&mut self) -> Option<T> {
        self.result.take()
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if matches!(self.status, TaskStatus::Completed | TaskStatus::Failed) {
            return Poll::Ready(());
        }

        self.status = TaskStatus::Running;
        let poll = catch_unwind(AssertUnwindSafe(|| {
            self.future
                .as_mut()
                .expect("pending state must own its future")
                .as_mut()
                .poll(cx)
        }));

        match poll {
            Ok(Poll::Pending) => {
                self.status = TaskStatus::Pending;
                Poll::Pending
            }
            Ok(Poll::Ready(result)) => {
                self.future = None;
                self.result = Some(result);
                self.status = TaskStatus::Completed;
                Poll::Ready(())
            }
            Err(payload) => {
                self.future = None;
                self.status = TaskStatus::Failed;
                resume_unwind(payload)
            }
        }
    }

    pub fn run(&mut self) {
        block_on(std::future::poll_fn(|cx| self.poll(cx)));
    }
}

struct ReadyFlag(AtomicBool);

impl Wake for ReadyFlag {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }
}

type LocalTaskFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

struct LocalTask {
    future: LocalTaskFuture,
    ready: Arc<ReadyFlag>,
}

/// A manually driven executor for futures that stay on the current thread.
#[derive(Default)]
pub struct LocalWorker {
    tasks: Vec<LocalTask>,
}

impl LocalWorker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + 'static,
    {
        self.tasks.push(LocalTask {
            future: Box::pin(future),
            ready: Arc::new(ReadyFlag(AtomicBool::new(true))),
        });
    }

    /// Polls runnable tasks until none are ready and returns the number completed.
    pub fn run_until_stalled(&mut self) -> usize {
        let mut completed = 0;
        loop {
            let mut polled = false;
            let mut index = 0;
            while index < self.tasks.len() {
                let task = &mut self.tasks[index];
                if !task.ready.0.swap(false, Ordering::AcqRel) {
                    index += 1;
                    continue;
                }

                polled = true;
                let waker = Waker::from(Arc::clone(&task.ready));
                let mut context = Context::from_waker(&waker);
                if task.future.as_mut().poll(&mut context).is_ready() {
                    self.tasks.swap_remove(index);
                    completed += 1;
                } else if !self.tasks[index].ready.0.load(Ordering::Acquire) {
                    index += 1;
                }
            }

            if !polled
                || !self
                    .tasks
                    .iter()
                    .any(|task| task.ready.0.load(Ordering::Acquire))
            {
                return completed;
            }
        }
    }

    pub fn pending_tasks(&self) -> usize {
        self.tasks.len()
    }
}

pub trait AsyncWorker {
    fn queue<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + 'static;
}

impl AsyncWorker for LocalWorker {
    fn queue<F>(&mut self, future: F)
    where
        F: Future<Output = ()> + 'static,
    {
        LocalWorker::queue(self, future);
    }
}

struct ThreadWake(thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Drives a future on the current thread, parking between wakeups.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);

    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        thread::park();
    }
}
