//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use crossbeam_channel::{Sender, unbounded};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;

const PENDING: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETED: u8 = 2;
const FAILED: u8 = 3;
const CANCELLED: u8 = 4;
const MAX_POOL_THREADS: usize = 8;
const MAX_POOL_TASKS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskStatus {
    Pending = PENDING,
    Running = RUNNING,
    Completed = COMPLETED,
    Failed = FAILED,
    Cancelled = CANCELLED,
}

impl TaskStatus {
    fn from_raw(value: u8) -> Self {
        match value {
            RUNNING => Self::Running,
            COMPLETED => Self::Completed,
            FAILED => Self::Failed,
            CANCELLED => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct WorkerPool {
    sender: Sender<Job>,
    active_tasks: AtomicUsize,
}

impl WorkerPool {
    fn reserve_task(&'static self) -> Result<TaskPermit, TaskLimitReached> {
        let mut active = self.active_tasks.load(Ordering::Acquire);
        loop {
            if active >= MAX_POOL_TASKS {
                return Err(TaskLimitReached);
            }
            match self.active_tasks.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(TaskPermit(self)),
                Err(actual) => active = actual,
            }
        }
    }
}

struct TaskPermit(&'static WorkerPool);

impl Drop for TaskPermit {
    fn drop(&mut self) {
        self.0.active_tasks.fetch_sub(1, Ordering::Release);
    }
}

/// Returned when the worker already has the maximum number of outstanding tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskLimitReached;

impl std::fmt::Display for TaskLimitReached {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("async worker task limit reached")
    }
}

impl std::error::Error for TaskLimitReached {}

fn worker_pool() -> &'static WorkerPool {
    static POOL: OnceLock<WorkerPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let worker_count = thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .clamp(1, MAX_POOL_THREADS);
        let (sender, receiver) = unbounded::<Job>();

        for index in 0..worker_count {
            let receiver = receiver.clone();
            thread::Builder::new()
                .name(format!("vagus-worker-{index}"))
                .spawn(move || {
                    loop {
                        let job = receiver.recv();
                        match job {
                            Ok(job) => {
                                let _ = catch_unwind(AssertUnwindSafe(job));
                            }
                            Err(_) => break,
                        }
                    }
                })
                .expect("failed to start async worker thread");
        }

        WorkerPool {
            sender,
            active_tasks: AtomicUsize::new(0),
        }
    })
}

type WorkerFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

enum TaskPoll {
    Pending,
    Ready,
    Cancelled,
}

struct WorkerTask<T> {
    future: Mutex<Option<WorkerFuture>>,
    shared: Arc<Shared<T>>,
    sender: Sender<Job>,
    permit: Mutex<Option<TaskPermit>>,
    scheduler: Mutex<SchedulerState>,
    cancel_requested: AtomicBool,
}

#[derive(Default)]
struct SchedulerState {
    queued: bool,
    running: bool,
    terminal: bool,
    wake_pending: bool,
}

type WorkerTaskParts<T> = (Arc<Shared<T>>, Arc<WorkerTask<T>>);

impl<T: Send + 'static> WorkerTask<T> {
    fn release_permit(&self) {
        lock(&self.permit).take();
    }

    fn schedule(self: &Arc<Self>) {
        let mut scheduler = lock(&self.scheduler);
        if scheduler.terminal {
            return;
        }
        if scheduler.running {
            scheduler.wake_pending = true;
            return;
        }
        if scheduler.queued {
            return;
        }

        scheduler.queued = true;
        let task = Arc::clone(self);
        let job: Job = Box::new(move || task.run());
        if self.sender.send(job).is_err() {
            scheduler.queued = false;
        }
    }

    fn finish_running(&self, terminal: bool) -> bool {
        let mut scheduler = lock(&self.scheduler);
        scheduler.running = false;
        if terminal {
            scheduler.terminal = true;
            scheduler.wake_pending = false;
            false
        } else {
            std::mem::take(&mut scheduler.wake_pending)
        }
    }

    fn run(self: Arc<Self>) {
        {
            let mut scheduler = lock(&self.scheduler);
            if scheduler.terminal {
                return;
            }
            scheduler.queued = false;
            scheduler.running = true;
            scheduler.wake_pending = false;
        }

        let waker = Waker::from(Arc::clone(&self));
        let mut context = Context::from_waker(&waker);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut future_slot = lock(&self.future);
            if future_slot.is_none() {
                return None;
            }
            if self.cancel_requested.load(Ordering::Acquire) {
                future_slot.take();
                return Some(TaskPoll::Cancelled);
            }
            self.shared.status.store(RUNNING, Ordering::Release);
            let poll = future_slot
                .as_mut()
                .expect("checked task future")
                .as_mut()
                .poll(&mut context);
            Some(match poll {
                Poll::Ready(()) => {
                    future_slot.take();
                    TaskPoll::Ready
                }
                Poll::Pending => {
                    self.shared.status.store(PENDING, Ordering::Release);
                    TaskPoll::Pending
                }
            })
        }));

        match outcome {
            Ok(Some(TaskPoll::Ready)) => {
                self.finish_running(true);
                self.release_permit();
                self.shared.status.store(COMPLETED, Ordering::Release);
                self.shared.wake_joiner();
            }
            Ok(Some(TaskPoll::Pending)) => {
                if self.finish_running(false) {
                    self.schedule();
                }
            }
            Ok(Some(TaskPoll::Cancelled)) => {
                self.finish_running(true);
                self.release_permit();
                self.shared.status.store(CANCELLED, Ordering::Release);
                self.shared.wake_joiner();
            }
            Ok(None) => {
                self.finish_running(true);
                self.release_permit();
            }
            Err(payload) => {
                let mut future = lock(&self.future);
                let _ = catch_unwind(AssertUnwindSafe(|| drop(future.take())));
                *lock(&self.shared.panic) = Some(payload);
                self.finish_running(true);
                self.release_permit();
                self.shared.status.store(FAILED, Ordering::Release);
                drop(future);
                self.shared.wake_joiner();
            }
        }
    }

    fn cancel(self: &Arc<Self>) -> bool {
        let status = TaskStatus::from_raw(self.shared.status.load(Ordering::Acquire));
        if matches!(
            status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return false;
        }
        if self
            .cancel_requested
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.schedule();
        true
    }
}

impl<T: Send + 'static> Wake for WorkerTask<T> {
    fn wake(self: Arc<Self>) {
        self.schedule();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.schedule();
    }
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
        match TaskStatus::from_raw(shared.status.load(Ordering::Acquire)) {
            TaskStatus::Completed => {
                if let Some(result) = lock(&shared.result).take() {
                    return Poll::Ready(result);
                }
            }
            TaskStatus::Failed => {
                if let Some(payload) = lock(&shared.panic).take() {
                    resume_unwind(payload);
                }
            }
            _ => {}
        }

        *lock(&shared.waker) = Some(cx.waker().clone());

        match TaskStatus::from_raw(shared.status.load(Ordering::Acquire)) {
            TaskStatus::Completed => lock(&shared.result)
                .take()
                .map_or(Poll::Pending, Poll::Ready),
            TaskStatus::Failed => {
                if let Some(payload) = lock(&shared.panic).take() {
                    resume_unwind(payload);
                }
                Poll::Pending
            }
            _ => Poll::Pending,
        }
    }
}

/// Cancellation outcome for a task spawned with [`Worker::spawn_cancellable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCancelled;

impl std::fmt::Display for TaskCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("task was cancelled")
    }
}

impl std::error::Error for TaskCancelled {}

/// A join handle whose task can be cancelled cooperatively.
pub struct CancellableJoinHandle<T> {
    shared: Arc<Shared<T>>,
    task: Arc<WorkerTask<T>>,
}

impl<T: Send + 'static> CancellableJoinHandle<T> {
    /// Requests cancellation; completion can win if it races this request.
    /// Returns `false` if cancellation was already requested or the task is terminal.
    pub fn cancel(&self) -> bool {
        self.task.cancel()
    }

    /// Returns the task's current status.
    pub fn status(&self) -> TaskStatus {
        TaskStatus::from_raw(self.shared.status.load(Ordering::Acquire))
    }
}

impl<T> Future for CancellableJoinHandle<T> {
    type Output = Result<T, TaskCancelled>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let shared = &self.get_mut().shared;
        match TaskStatus::from_raw(shared.status.load(Ordering::Acquire)) {
            TaskStatus::Completed => {
                if let Some(result) = lock(&shared.result).take() {
                    return Poll::Ready(Ok(result));
                }
            }
            TaskStatus::Failed => {
                if let Some(payload) = lock(&shared.panic).take() {
                    resume_unwind(payload);
                }
            }
            TaskStatus::Cancelled => return Poll::Ready(Err(TaskCancelled)),
            _ => {}
        }

        *lock(&shared.waker) = Some(cx.waker().clone());

        match TaskStatus::from_raw(shared.status.load(Ordering::Acquire)) {
            TaskStatus::Completed => lock(&shared.result)
                .take()
                .map_or(Poll::Pending, |result| Poll::Ready(Ok(result))),
            TaskStatus::Failed => {
                if let Some(payload) = lock(&shared.panic).take() {
                    resume_unwind(payload);
                }
                Poll::Pending
            }
            TaskStatus::Cancelled => Poll::Ready(Err(TaskCancelled)),
            _ => Poll::Pending,
        }
    }
}

/// Polls `Send` futures on a global pool of at most eight reusable threads.
#[derive(Debug, Default, Clone, Copy)]
pub struct Worker;

impl Worker {
    pub const fn new() -> Self {
        Self
    }

    fn create_task<F>(future: F, permit: TaskPermit) -> WorkerTaskParts<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let shared = Arc::new(Shared::new());
        let task_shared = Arc::clone(&shared);
        let task = Arc::new(WorkerTask {
            future: Mutex::new(Some(Box::pin(async move {
                let result = future.await;
                *lock(&task_shared.result) = Some(result);
            }))),
            shared: Arc::clone(&shared),
            sender: worker_pool().sender.clone(),
            permit: Mutex::new(Some(permit)),
            scheduler: Mutex::new(SchedulerState::default()),
            cancel_requested: AtomicBool::new(false),
        });
        (shared, task)
    }

    /// Spawns a task unless the global pool already has 1024 outstanding tasks.
    pub fn spawn<F>(&self, future: F) -> Result<JoinHandle<F::Output>, TaskLimitReached>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let permit = worker_pool().reserve_task()?;
        let (shared, task) = Self::create_task(future, permit);
        task.schedule();
        Ok(JoinHandle { shared })
    }

    /// Spawns a cancellable task, subject to the same global task limit as [`Worker::spawn`].
    pub fn spawn_cancellable<F>(
        &self,
        future: F,
    ) -> Result<CancellableJoinHandle<F::Output>, TaskLimitReached>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let permit = worker_pool().reserve_task()?;
        let (shared, task) = Self::create_task(future, permit);
        task.schedule();
        Ok(CancellableJoinHandle { shared, task })
    }
}

/// Alias for the bounded worker-pool executor.
pub type Thread = Worker;

/// Common spawn interface for executors that run `Send` futures.
pub trait AsyncExecutor {
    fn spawn<F>(&self, future: F) -> Result<JoinHandle<F::Output>, TaskLimitReached>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static;
}

impl AsyncExecutor for Worker {
    fn spawn<F>(&self, future: F) -> Result<JoinHandle<F::Output>, TaskLimitReached>
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

    fn discard_future(&mut self) -> Result<(), Box<dyn Any + Send>> {
        catch_unwind(AssertUnwindSafe(|| drop(self.future.take())))
    }

    /// Cancels this task and immediately drops its future.
    pub fn cancel(&mut self) -> bool {
        if matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            return false;
        }

        self.status = TaskStatus::Cancelled;
        self.result = None;
        if let Err(payload) = self.discard_future() {
            self.status = TaskStatus::Failed;
            resume_unwind(payload);
        }
        true
    }

    pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
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
                if let Err(payload) = self.discard_future() {
                    self.status = TaskStatus::Failed;
                    resume_unwind(payload);
                }
                self.result = Some(result);
                self.status = TaskStatus::Completed;
                Poll::Ready(())
            }
            Err(payload) => {
                self.status = TaskStatus::Failed;
                let _ = self.discard_future();
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

struct LocalOutputFuture<F, T> {
    future: Option<Pin<Box<F>>>,
    state: Rc<RefCell<LocalTaskState<T>>>,
}

impl<F: Future<Output = T>, T> Future for LocalOutputFuture<F, T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Some(future) = this.future.as_mut() else {
            return Poll::Ready(());
        };

        this.state.borrow_mut().status = TaskStatus::Running;
        match future.as_mut().poll(cx) {
            Poll::Pending => {
                this.state.borrow_mut().status = TaskStatus::Pending;
                Poll::Pending
            }
            Poll::Ready(result) => {
                drop(this.future.take());
                let mut state = this.state.borrow_mut();
                state.result = Some(result);
                state.status = TaskStatus::Completed;
                Poll::Ready(())
            }
        }
    }
}

struct LocalTask {
    future: LocalTaskFuture,
    ready: Arc<ReadyFlag>,
    cancel_requested: Rc<Cell<bool>>,
    on_panic: Option<Box<dyn FnOnce()>>,
    on_cancel: Option<Box<dyn FnOnce()>>,
}

struct LocalTaskState<T> {
    result: Option<T>,
    status: TaskStatus,
}

/// Retrieves a local task's completion status and result.
pub struct LocalJoinHandle<T> {
    state: Rc<RefCell<LocalTaskState<T>>>,
    cancel_requested: Rc<Cell<bool>>,
    ready: Arc<ReadyFlag>,
}

impl<T> LocalJoinHandle<T> {
    /// Returns the task's current status.
    pub fn status(&self) -> TaskStatus {
        self.state.borrow().status
    }

    /// Takes the completed result once, returning `None` while pending or afterward.
    pub fn take_result(&self) -> Option<T> {
        self.state.borrow_mut().result.take()
    }

    /// Requests cancellation; the future is dropped when its worker is next driven.
    pub fn cancel(&self) -> bool {
        if matches!(
            self.state.borrow().status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) || self.cancel_requested.replace(true)
        {
            return false;
        }

        self.ready.0.store(true, Ordering::Release);
        true
    }
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
        self.enqueue(Box::pin(future), Rc::new(Cell::new(false)), None, None);
    }

    fn enqueue(
        &mut self,
        future: LocalTaskFuture,
        cancel_requested: Rc<Cell<bool>>,
        on_panic: Option<Box<dyn FnOnce()>>,
        on_cancel: Option<Box<dyn FnOnce()>>,
    ) -> Arc<ReadyFlag> {
        let ready = Arc::new(ReadyFlag(AtomicBool::new(true)));
        self.tasks.push(LocalTask {
            future,
            ready: Arc::clone(&ready),
            cancel_requested,
            on_panic,
            on_cancel,
        });
        ready
    }

    /// Queues a task and returns a handle for its status and result.
    /// The future runs when this worker is driven with `run_until_stalled`.
    pub fn spawn<F>(&mut self, future: F) -> LocalJoinHandle<F::Output>
    where
        F: Future + 'static,
        F::Output: 'static,
    {
        let state = Rc::new(RefCell::new(LocalTaskState {
            result: None,
            status: TaskStatus::Pending,
        }));
        let task_state = Rc::clone(&state);
        let failed_state = Rc::clone(&state);
        let on_panic = Box::new(move || {
            failed_state.borrow_mut().status = TaskStatus::Failed;
        });
        let cancelled_state = Rc::clone(&state);
        let on_cancel = Box::new(move || {
            cancelled_state.borrow_mut().status = TaskStatus::Cancelled;
        });
        let cancel_requested = Rc::new(Cell::new(false));
        let ready = self.enqueue(
            Box::pin(LocalOutputFuture {
                future: Some(Box::pin(future)),
                state: task_state,
            }),
            Rc::clone(&cancel_requested),
            Some(on_panic),
            Some(on_cancel),
        );
        LocalJoinHandle {
            state,
            cancel_requested,
            ready,
        }
    }

    /// Polls runnable tasks until none are ready and returns the number completed.
    pub fn run_until_stalled(&mut self) -> usize {
        let mut completed = 0;
        loop {
            let mut polled = false;
            let mut index = 0;
            while index < self.tasks.len() {
                if self.tasks[index].cancel_requested.get() {
                    polled = true;
                    let mut task = self.tasks.swap_remove(index);
                    if let Some(on_cancel) = task.on_cancel.take() {
                        on_cancel();
                    }
                    let drop_result = catch_unwind(AssertUnwindSafe(|| drop(task.future)));
                    if let Err(payload) = drop_result {
                        if let Some(on_panic) = task.on_panic.take() {
                            on_panic();
                        }
                        resume_unwind(payload);
                    }
                    continue;
                }

                if !self.tasks[index].ready.0.swap(false, Ordering::AcqRel) {
                    index += 1;
                    continue;
                }

                polled = true;
                let poll_result = {
                    let task = &mut self.tasks[index];
                    let waker = Waker::from(Arc::clone(&task.ready));
                    let mut context = Context::from_waker(&waker);
                    catch_unwind(AssertUnwindSafe(|| task.future.as_mut().poll(&mut context)))
                };
                match poll_result {
                    Ok(Poll::Ready(())) => {
                        self.tasks.swap_remove(index);
                        completed += 1;
                    }
                    Ok(Poll::Pending) => {
                        if !self.tasks[index].ready.0.load(Ordering::Acquire) {
                            index += 1;
                        }
                    }
                    Err(payload) => {
                        if let Some(on_panic) = self.tasks[index].on_panic.take() {
                            on_panic();
                        }
                        let task = self.tasks.swap_remove(index);
                        let _ = catch_unwind(AssertUnwindSafe(|| drop(task.future)));
                        resume_unwind(payload);
                    }
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

impl Drop for LocalWorker {
    fn drop(&mut self) {
        let mut panic_payload = None;
        for mut task in self.tasks.drain(..) {
            if let Some(on_cancel) = task.on_cancel.take() {
                on_cancel();
            }

            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(task.future))) {
                if let Some(on_panic) = task.on_panic.take() {
                    on_panic();
                }
                if panic_payload.is_none() {
                    panic_payload = Some(payload);
                }
            }
        }

        if !std::thread::panicking() {
            if let Some(payload) = panic_payload {
                resume_unwind(payload);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::{
        LocalWorker, MAX_POOL_TASKS, MAX_POOL_THREADS, State, TaskCancelled, TaskStatus, Worker,
        block_on,
    };
    use std::cell::Cell;
    use std::collections::HashSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::task::Waker;
    use std::time::{Duration, Instant};

    static WORKER_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn local_worker_exposes_task_status_and_result() {
        let mut worker = LocalWorker::new();
        let handle = worker.spawn(async { std::rc::Rc::new(42) });

        assert_eq!(handle.status(), TaskStatus::Pending);
        assert_eq!(worker.run_until_stalled(), 1);
        assert_eq!(handle.status(), TaskStatus::Completed);
        assert_eq!(handle.take_result().map(|value| *value), Some(42));
        assert_eq!(handle.take_result().map(|value| *value), None);
    }

    #[test]
    fn dropping_local_worker_cancels_pending_task_handles() {
        struct DropSignal(Rc<Cell<bool>>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let signal = DropSignal(Rc::clone(&dropped));
        let mut worker = LocalWorker::new();
        let handle = worker.spawn(async move {
            let _signal = signal;
            std::future::pending::<Rc<usize>>().await
        });

        assert_eq!(handle.status(), TaskStatus::Pending);
        drop(worker);

        assert_eq!(handle.status(), TaskStatus::Cancelled);
        assert!(handle.take_result().is_none());
        assert!(dropped.get());
    }

    #[test]
    fn local_worker_cancels_pending_task_and_drops_its_future() {
        struct DropSignal(Rc<Cell<bool>>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let polls = Rc::new(Cell::new(0));
        let dropped = Rc::new(Cell::new(false));
        let task_polls = Rc::clone(&polls);
        let signal = DropSignal(Rc::clone(&dropped));
        let future = std::future::poll_fn(move |_| {
            let _keep_signal_alive = &signal;
            task_polls.set(task_polls.get() + 1);
            std::task::Poll::<()>::Pending
        });
        let mut worker = LocalWorker::new();
        let handle = worker.spawn(future);

        assert_eq!(worker.run_until_stalled(), 0);
        assert_eq!(polls.get(), 1);
        assert_eq!(handle.status(), TaskStatus::Pending);
        assert!(handle.cancel());
        assert_eq!(worker.run_until_stalled(), 0);
        assert_eq!(handle.status(), TaskStatus::Cancelled);
        assert_eq!(worker.pending_tasks(), 0);
        assert!(dropped.get());
        assert!(!handle.cancel());
    }

    #[test]
    fn state_cancellation_drops_future_and_is_terminal() {
        struct DropSignal(Rc<Cell<bool>>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }

        let dropped = Rc::new(Cell::new(false));
        let signal = DropSignal(Rc::clone(&dropped));
        let mut state = State::new(async move {
            let _keep_signal_alive = signal;
            std::future::pending::<()>().await;
        });

        assert!(state.cancel());
        assert_eq!(state.status(), TaskStatus::Cancelled);
        assert_eq!(state.result(), None);
        assert!(dropped.get());
        assert!(!state.cancel());
    }

    #[test]
    fn local_worker_marks_panicking_task_failed() {
        let mut worker = LocalWorker::new();
        let handle = worker.spawn(async { panic!("local task failed") });

        assert!(catch_unwind(AssertUnwindSafe(|| worker.run_until_stalled())).is_err());
        assert_eq!(handle.status(), TaskStatus::Failed);
    }

    #[test]
    fn worker_reuses_a_bounded_set_of_threads() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        const TASK_COUNT: usize = 32;
        let worker = Worker::new();
        let mut handles = (0..TASK_COUNT)
            .map(|_| {
                worker
                    .spawn(async {
                        std::thread::sleep(Duration::from_millis(40));
                        std::thread::current().id()
                    })
                    .expect("task capacity available")
            })
            .collect::<Vec<_>>();

        let thread_ids = handles.iter_mut().map(block_on).collect::<HashSet<_>>();

        assert!(thread_ids.len() <= MAX_POOL_THREADS);
        assert!(thread_ids.len() < TASK_COUNT);
    }

    #[test]
    fn worker_runs_tasks_concurrently_up_to_available_pool_parallelism() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        let pool_workers = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .min(MAX_POOL_THREADS);
        if pool_workers < 2 {
            eprintln!("SKIP: worker-poll overlap requires at least two available workers");
            return;
        }

        let active = Arc::new(AtomicUsize::new(0));
        let peak_active = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(pool_workers);
        let worker = Worker::new();
        let mut handles = (0..pool_workers)
            .map(|_| {
                let active = Arc::clone(&active);
                let peak_active = Arc::clone(&peak_active);
                let release = Arc::clone(&release);
                let started_tx = started_tx.clone();
                worker
                    .spawn(async move {
                        let now_active = active.fetch_add(1, Ordering::AcqRel) + 1;
                        peak_active.fetch_max(now_active, Ordering::AcqRel);
                        started_tx
                            .send(())
                            .expect("test is waiting for worker starts");
                        while !release.load(Ordering::Acquire) {
                            std::thread::yield_now();
                        }
                        active.fetch_sub(1, Ordering::AcqRel);
                    })
                    .expect("task capacity available")
            })
            .collect::<Vec<_>>();
        drop(started_tx);

        let all_started =
            (0..pool_workers).all(|_| started_rx.recv_timeout(Duration::from_secs(2)).is_ok());
        release.store(true, Ordering::Release);
        for handle in &mut handles {
            block_on(handle);
        }

        assert!(all_started, "each pool worker should poll one blocked task");
        assert_eq!(peak_active.load(Ordering::Acquire), pool_workers);
    }

    #[test]
    fn worker_reschedules_woken_tasks_and_returns_the_result() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        let polls = Arc::new(AtomicUsize::new(0));
        let task_polls = Arc::clone(&polls);
        let future = std::future::poll_fn(move |cx| {
            if task_polls.fetch_add(1, Ordering::Relaxed) == 0 {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(73)
            }
        });
        let mut join = Worker::new()
            .spawn(future)
            .expect("task capacity available");

        assert_eq!(block_on(&mut join), 73);
        assert_eq!(polls.load(Ordering::Relaxed), 2);
        assert_eq!(join.status(), TaskStatus::Completed);
    }

    #[test]
    fn worker_marks_panicking_tasks_failed_and_propagates_the_panic() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        let mut join = Worker::new()
            .spawn(async { panic!("worker task failed") })
            .expect("task capacity available");

        assert!(catch_unwind(AssertUnwindSafe(|| block_on(&mut join))).is_err());
        assert_eq!(join.status(), TaskStatus::Failed);
    }

    #[test]
    fn worker_cancels_a_pending_task_and_drops_its_future() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        struct DropSignal(Arc<AtomicBool>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let polls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let task_polls = Arc::clone(&polls);
        let signal = DropSignal(Arc::clone(&dropped));
        let future = std::future::poll_fn(move |_| {
            let _keep_signal_alive = &signal;
            task_polls.fetch_add(1, Ordering::Release);
            std::task::Poll::<()>::Pending
        });
        let mut join = Worker::new()
            .spawn_cancellable(future)
            .expect("task capacity available");
        let deadline = Instant::now() + Duration::from_secs(2);
        while polls.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(polls.load(Ordering::Acquire), 1);

        assert!(join.cancel());
        assert_eq!(block_on(&mut join), Err(TaskCancelled));
        assert_eq!(join.status(), TaskStatus::Cancelled);
        assert!(dropped.load(Ordering::Acquire));
    }

    #[test]
    fn worker_cancels_a_running_task_once_and_drops_its_future() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        struct DropSignal(Arc<AtomicBool>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let polls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let task_polls = Arc::clone(&polls);
        let signal = DropSignal(Arc::clone(&dropped));
        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        let future = std::future::poll_fn(move |_| {
            let _keep_signal_alive = &signal;
            task_polls.fetch_add(1, Ordering::Release);
            started_tx.send(()).expect("test is waiting for the poll");
            assert!(resume_rx.recv_timeout(Duration::from_secs(5)).is_ok());
            std::task::Poll::<()>::Pending
        });
        let mut join = Worker::new()
            .spawn_cancellable(future)
            .expect("task capacity available");

        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("worker did not start polling the task");
        assert_eq!(join.status(), TaskStatus::Running);
        let first_cancel = join.cancel();
        let duplicate_cancel = join.cancel();
        resume_tx
            .send(())
            .expect("worker poll is waiting to resume");

        assert!(first_cancel);
        assert!(!duplicate_cancel, "a duplicate cancellation is not new");
        assert_eq!(block_on(&mut join), Err(TaskCancelled));
        assert_eq!(join.status(), TaskStatus::Cancelled);
        assert_eq!(polls.load(Ordering::Acquire), 1);
        assert!(dropped.load(Ordering::Acquire));
    }

    #[test]
    fn join_handle_waits_for_terminal_status_before_exposing_result() {
        let shared = Arc::new(super::Shared::new());
        *super::lock(&shared.result) = Some(42);
        let mut handle = super::JoinHandle {
            shared: Arc::clone(&shared),
        };
        let mut context = std::task::Context::from_waker(Waker::noop());

        assert!(matches!(
            std::pin::Pin::new(&mut handle).poll(&mut context),
            std::task::Poll::Pending
        ));

        shared.status.store(super::COMPLETED, Ordering::Release);
        assert!(matches!(
            std::pin::Pin::new(&mut handle).poll(&mut context),
            std::task::Poll::Ready(42)
        ));
    }

    #[test]
    fn cancellable_join_handle_waits_for_terminal_status_before_exposing_result() {
        let shared = Arc::new(super::Shared::new());
        *super::lock(&shared.result) = Some(17);
        let (sender, _receiver) = super::unbounded::<super::Job>();
        let task = Arc::new(super::WorkerTask {
            future: Mutex::new(None),
            shared: Arc::clone(&shared),
            sender,
            permit: Mutex::new(None),
            scheduler: Mutex::new(super::SchedulerState::default()),
            cancel_requested: AtomicBool::new(false),
        });
        let mut handle = super::CancellableJoinHandle { shared: Arc::clone(&shared), task };
        let mut context = std::task::Context::from_waker(Waker::noop());

        assert!(matches!(
            std::pin::Pin::new(&mut handle).poll(&mut context),
            std::task::Poll::Pending
        ));

        shared.status.store(super::COMPLETED, Ordering::Release);
        assert!(matches!(
            std::pin::Pin::new(&mut handle).poll(&mut context),
            std::task::Poll::Ready(Ok(17))
        ));
    }

    #[test]
    fn completed_cancellable_task_returns_value_and_rejects_cancellation() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        let mut join = Worker::new()
            .spawn_cancellable(async { 42 })
            .expect("task capacity available");

        assert_eq!(block_on(&mut join), Ok(42));
        assert_eq!(join.status(), TaskStatus::Completed);
        assert!(!join.cancel());
    }

    #[test]
    fn worker_rejects_tasks_when_the_outstanding_task_limit_is_reached() {
        let _serial = WORKER_TEST_LOCK.lock().unwrap();
        const TASK_LIMIT: usize = MAX_POOL_TASKS;
        let worker = Worker::new();
        let ready = Arc::new(AtomicBool::new(false));
        let wakers = Arc::new(Mutex::new(Vec::<Waker>::new()));
        let mut handles = Vec::with_capacity(TASK_LIMIT);

        for _ in 0..TASK_LIMIT {
            let task_ready = Arc::clone(&ready);
            let task_wakers = Arc::clone(&wakers);
            let future = std::future::poll_fn(move |cx| {
                if task_ready.load(Ordering::Acquire) {
                    std::task::Poll::Ready(())
                } else {
                    task_wakers.lock().unwrap().push(cx.waker().clone());
                    std::task::Poll::Pending
                }
            });
            handles.push(
                worker
                    .spawn_cancellable(future)
                    .expect("task capacity available"),
            );
        }

        assert!(matches!(
            worker.spawn(async {}),
            Err(super::TaskLimitReached)
        ));

        let mut cancelled = handles.pop().expect("a task was admitted");
        assert!(cancelled.cancel());
        assert_eq!(block_on(&mut cancelled), Err(super::TaskCancelled));

        let mut replacement = worker
            .spawn(async { 9 })
            .expect("cancelled tasks release their capacity");
        assert_eq!(block_on(&mut replacement), 9);

        let mut failed = worker
            .spawn(async { panic!("failed task") })
            .expect("completed replacement releases its capacity");
        assert!(catch_unwind(AssertUnwindSafe(|| block_on(&mut failed))).is_err());
        assert_eq!(failed.status(), TaskStatus::Failed);

        let mut after_failure = worker
            .spawn(async { 11 })
            .expect("failed tasks release their capacity");
        assert_eq!(block_on(&mut after_failure), 11);

        ready.store(true, Ordering::Release);
        for waker in wakers.lock().unwrap().drain(..) {
            waker.wake();
        }
        for mut handle in handles {
            assert_eq!(block_on(&mut handle), Ok(()));
        }

        let mut next = worker
            .spawn(async { 17 })
            .expect("completed tasks release their capacity");
        assert_eq!(block_on(&mut next), 17);
    }
}
