use std::{mem::MaybeUninit, sync::mpsc, time};

use gmod::{lua, task_queue::run_callbacks};
use tokio::runtime::{Builder, Runtime};
use tokio_util::task::TaskTracker;

use crate::{print_goobie, TASKS_WAITING_TIMEOUT};

static mut RUN_TIME: MaybeUninit<Runtime> = MaybeUninit::uninit();
static mut TASK_TRACKER: MaybeUninit<TaskTracker> = MaybeUninit::uninit();

pub(super) fn load(worker_threads: u16) {
    print_goobie!("Using {worker_threads} worker threads");

    let run_time = Builder::new_multi_thread()
        .worker_threads(worker_threads as usize)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let task_tracker = TaskTracker::new();

    unsafe {
        RUN_TIME = MaybeUninit::new(run_time);
        TASK_TRACKER = MaybeUninit::new(task_tracker);
    }
}

pub(super) fn unload() {
    let run_time = unsafe { RUN_TIME.assume_init_read() };

    let task_tracker = unsafe { TASK_TRACKER.assume_init_read() };
    task_tracker.close();

    if !task_tracker.is_empty() {
        print_goobie!(
            "Waiting up to {} seconds for {} pending tasks to complete...",
            TASKS_WAITING_TIMEOUT.as_secs(),
            task_tracker.len()
        );

        run_time.block_on(async {
            tokio::select! {
                _ = task_tracker.wait() => {
                    print_goobie!("All pending tasks have completed!");
                },
                _ = tokio::time::sleep(TASKS_WAITING_TIMEOUT) => {
                    print_goobie!("Timed out waiting for pending tasks to complete!");
                },
            }
        });
    }
}

fn read<'a>() -> &'a Runtime {
    unsafe { RUN_TIME.assume_init_ref() }
}

fn read_tracker<'a>() -> &'a TaskTracker {
    unsafe { TASK_TRACKER.assume_init_ref() }
}

pub fn run_async<F>(fut: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    read().spawn(read_tracker().track_future(fut))
}

// DO NOT CALL THIS INSIDE __gc OR YOU WILL GET A LOVELY PANIC, not certain why but i think
// because __gc shouldn't run more lua code? cant tell really but it def about __gc, as using this function
// in same scenario works fine, it's just __gc that panics
pub fn wait_async<F>(l: lua::State, fut: F) -> F::Output
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    // so previously, i used block_on but that would cause tokio to panic with:
    // "Cannot start a runtime from within a runtime. This happens because a function (like `block_on`)
    // attempted to block the current thread while the thread is being used to drive asynchronous tasks."
    // this would happen when mixing async with sync code
    let (tx, rx) = mpsc::sync_channel::<F::Output>(0); // 0 makes it a "rendezvous" channel

    run_async(async move {
        let res = fut.await;
        let _ = tx.send(res);
    });

    loop {
        // this will make sure that queries run properly
        // if a txn is running, it takes the lock till it's over, but if we are just blocking the main thread how would it finish?
        run_callbacks(l);
        if let Ok(res) = rx.recv_timeout(time::Duration::from_millis(50)) {
            return res;
        }
    }
}
