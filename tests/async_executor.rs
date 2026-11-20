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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use vagus::essential::r#async::{
    AsyncWorker, LocalJoinHandle, LocalWorker, State, TaskStatus, Thread, Worker, block_on,
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
fn local_worker_marks_completed_task_failed_if_future_cleanup_panics() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    let guard = PanicOnDrop;
    let mut worker = LocalWorker::new();
    let handle = worker.spawn(std::future::poll_fn(move |_| {
        let _keep_guard_alive = &guard;
        Poll::Ready(42)
    }));

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| worker.run_until_stalled()));

    assert!(result.is_err());
    assert_eq!(handle.status(), TaskStatus::Failed);
    assert!(handle.take_result().is_none());
}

#[test]
fn local_worker_marks_cancelled_task_failed_if_future_cleanup_panics() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    let guard = PanicOnDrop;
    let mut worker = LocalWorker::new();
    let handle = worker.spawn(std::future::poll_fn(move |_| {
        let _keep_guard_alive = &guard;
        Poll::Pending::<()>
    }));

    assert_eq!(worker.run_until_stalled(), 0);
    assert_eq!(handle.status(), TaskStatus::Pending);
    assert!(handle.cancel());

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| worker.run_until_stalled()));

    assert!(result.is_err());
    assert_eq!(handle.status(), TaskStatus::Failed);
    assert!(handle.take_result().is_none());
    assert_eq!(worker.pending_tasks(), 0);
    assert!(!handle.cancel());
}

#[test]
fn local_worker_preserves_poll_panic_when_future_cleanup_also_panics() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    struct PanicOnPoll {
        _guard: PanicOnDrop,
    }
    impl Future for PanicOnPoll {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            panic!("task poll failed");
        }
    }

    let mut worker = LocalWorker::new();
    let handle = worker.spawn(PanicOnPoll {
        _guard: PanicOnDrop,
    });

    let payload = std::panic::catch_unwind(AssertUnwindSafe(|| worker.run_until_stalled()))
        .expect_err("task poll panic should propagate");

    assert_eq!(
        payload.downcast_ref::<&str>().copied(),
        Some("task poll failed")
    );
    assert_eq!(handle.status(), TaskStatus::Failed);
}

#[test]
fn local_worker_returns_non_send_result_after_external_wake() {
    let stored_waker = Arc::new(Mutex::new(None));
    let future_waker = Arc::clone(&stored_waker);
    let local_result = Rc::new(73);
    let future_result = Rc::clone(&local_result);
    let mut first_poll = true;
    let mut worker = LocalWorker::new();
    let handle = worker.spawn(std::future::poll_fn(move |cx| {
        if first_poll {
            first_poll = false;
            *future_waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(Rc::clone(&future_result))
        }
    }));

    assert_eq!(worker.run_until_stalled(), 0);
    assert_eq!(handle.status(), TaskStatus::Pending);
    assert_eq!(worker.pending_tasks(), 1);
    assert!(handle.take_result().is_none());

    stored_waker.lock().unwrap().take().unwrap().wake();
    assert_eq!(worker.run_until_stalled(), 1);

    assert_eq!(handle.status(), TaskStatus::Completed);
    assert_eq!(worker.pending_tasks(), 0);
    assert_eq!(*handle.take_result().unwrap(), 73);
    assert!(handle.take_result().is_none());
}

#[test]
fn local_worker_reports_running_while_polling_a_task() {
    use std::cell::{Cell, RefCell};

    let handle_slot: Rc<RefCell<Option<LocalJoinHandle<()>>>> = Rc::new(RefCell::new(None));
    let task_handle_slot = Rc::clone(&handle_slot);
    let observed_status = Rc::new(Cell::new(TaskStatus::Pending));
    let task_observed_status = Rc::clone(&observed_status);
    let mut worker = LocalWorker::new();
    let handle = worker.spawn(std::future::poll_fn(move |_| {
        task_observed_status.set(
            task_handle_slot
                .borrow()
                .as_ref()
                .expect("task handle is installed before polling")
                .status(),
        );
        Poll::Ready(())
    }));
    *handle_slot.borrow_mut() = Some(handle);

    assert_eq!(worker.run_until_stalled(), 1);

    let handle = handle_slot.borrow_mut().take().unwrap();
    assert_eq!(observed_status.get(), TaskStatus::Running);
    assert_eq!(handle.status(), TaskStatus::Completed);
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
fn state_marks_completed_task_failed_if_future_cleanup_panics() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    let guard = PanicOnDrop;
    let mut state = State::new(std::future::poll_fn(move |_| {
        let _keep_guard_alive = &guard;
        Poll::Ready(42)
    }));

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| state.run()));

    assert!(result.is_err());
    assert_eq!(state.status(), TaskStatus::Failed);
    assert!(state.take_result().is_none());
}

#[test]
fn state_marks_cancelled_task_failed_if_future_cleanup_panics() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    let guard = PanicOnDrop;
    let mut state = State::new(std::future::poll_fn(move |_| {
        let _keep_guard_alive = &guard;
        Poll::Pending::<usize>
    }));

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| state.cancel()));

    assert!(result.is_err());
    assert_eq!(state.status(), TaskStatus::Failed);
    assert!(state.take_result().is_none());
}

#[test]
fn state_marks_task_failed_when_poll_and_future_cleanup_panic() {
    struct PanicOnDrop;
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    struct PanicOnPoll {
        _guard: PanicOnDrop,
    }
    impl Future for PanicOnPoll {
        type Output = usize;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            panic!("future poll failed");
        }
    }

    let mut state = State::new(PanicOnPoll {
        _guard: PanicOnDrop,
    });
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| state.run()));

    let payload = result.expect_err("task panics should propagate");
    assert_eq!(
        payload.downcast_ref::<&str>().copied(),
        Some("future poll failed")
    );
    assert_eq!(state.status(), TaskStatus::Failed);
    assert!(state.take_result().is_none());
}

#[test]
fn thread_is_a_queue_free_worker() {
    let thread = Thread::new();
    let mut join = thread.spawn(async { 31 }).expect("task capacity available");

    assert_eq!(block_on(&mut join), 31);
    assert_eq!(join.status(), TaskStatus::Completed);
}

#[test]
fn block_on_and_worker_join_return_the_task_result() {
    assert_eq!(block_on(WakeFromThread(false)), 13);
    assert_eq!(block_on(async { 7 }), 7);

    let worker = Worker::new();
    let mut join = worker.spawn(async { 42 }).expect("task capacity available");
    assert_eq!(block_on(&mut join), 42);
    assert_eq!(join.status(), TaskStatus::Completed);
}

#[test]
fn worker_records_failure_and_propagates_task_panics_on_join() {
    let worker = Worker::new();
    let mut join = worker
        .spawn(async { panic!("task failed") })
        .expect("task capacity available");
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| block_on(&mut join)));

    assert!(result.is_err());
    assert_eq!(join.status(), TaskStatus::Failed);
}

#[test]
fn worker_handles_do_not_return_results_when_future_cleanup_panics() {
    struct ReadyThenPanicOnDrop;
    impl Future for ReadyThenPanicOnDrop {
        type Output = usize;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Ready(42)
        }
    }
    impl Drop for ReadyThenPanicOnDrop {
        fn drop(&mut self) {
            panic!("future cleanup failed");
        }
    }

    let worker = Worker::new();
    let mut join = worker
        .spawn(ReadyThenPanicOnDrop)
        .expect("task capacity available");
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| block_on(&mut join)));

    let payload = result.expect_err("cleanup panic must not leak a task result");
    assert_eq!(
        payload.downcast_ref::<&str>().copied(),
        Some("future cleanup failed")
    );
    assert_eq!(join.status(), TaskStatus::Failed);

    let mut cancellable = worker
        .spawn_cancellable(ReadyThenPanicOnDrop)
        .expect("task capacity available");
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| block_on(&mut cancellable)));

    let payload = result.expect_err("cleanup panic must not leak a cancellable task result");
    assert_eq!(
        payload.downcast_ref::<&str>().copied(),
        Some("future cleanup failed")
    );
    assert_eq!(cancellable.status(), TaskStatus::Failed);
}
