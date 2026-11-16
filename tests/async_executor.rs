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

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

use vagus::essential::r#async::{
    AsyncWorker, LocalWorker, State, TaskStatus, Thread, Worker, block_on,
};

struct YieldOnce(Arc<AtomicUsize>);

impl Future for YieldOnce {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

struct WakeFromThread(bool);

impl Future for WakeFromThread {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0 {
            Poll::Ready(13)
        } else {
            self.0 = true;
            let waker = cx.waker().clone();
            std::thread::spawn(move || waker.wake());
            Poll::Pending
        }
    }
}

#[test]
fn local_worker_repolls_woken_tasks_until_stalled() {
    let polls = Arc::new(AtomicUsize::new(0));
    let mut worker = LocalWorker::new();
    AsyncWorker::queue(&mut worker, YieldOnce(Arc::clone(&polls)));

    assert_eq!(worker.run_until_stalled(), 1);
    assert_eq!(polls.load(Ordering::Relaxed), 2);
    assert_eq!(worker.pending_tasks(), 0);
}

#[test]
fn state_drives_one_non_send_future_without_a_queue() {
    let local_value = Rc::new(42);
    let mut state = State::new(async move { local_value });

    assert_eq!(state.status(), TaskStatus::Pending);
    state.run();
    assert_eq!(state.status(), TaskStatus::Completed);
    assert_eq!(**state.result().unwrap(), 42);
    assert_eq!(state.take_result().map(|value| *value), Some(42));
}

#[test]
fn thread_is_a_queue_free_worker() {
    let thread = Thread::new();
    let mut join = thread.spawn(async { 31 });

    assert_eq!(block_on(&mut join), 31);
    assert_eq!(join.status(), TaskStatus::Completed);
}

#[test]
fn block_on_and_worker_join_return_the_task_result() {
    assert_eq!(block_on(WakeFromThread(false)), 13);
    assert_eq!(block_on(async { 7 }), 7);

    let worker = Worker::new();
    let mut join = worker.spawn(async { 42 });
    assert_eq!(block_on(&mut join), 42);
    assert_eq!(join.status(), TaskStatus::Completed);
}

#[test]
fn worker_records_failure_and_propagates_task_panics_on_join() {
    let worker = Worker::new();
    let mut join = worker.spawn(async { panic!("task failed") });
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| block_on(&mut join)));

    assert!(result.is_err());
    assert_eq!(join.status(), TaskStatus::Failed);
}
