use std::{
    future::Future,
    mem::forget,
    pin::Pin,
    sync::{mpsc::{self}, Arc, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker}, time::Duration,
};

// export
pub mod net;
pub mod reactor;

#[macro_export]
#[allow(unused_macros)]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

type TopFuture = Pin<Box<dyn Future<Output = ()>>>;
pub struct Runtime {}

impl Runtime {
    pub fn run(&self, mut futures: Vec<Box<dyn Future<Output = ()> + Send + 'static>>) {
        // --- spawn reactor and set reactor's context to current thread ---
        reactor::enter();

        // channel as task queue:
        // Executor holds receiver, Waker holds the sender for sending Task
        let (sender, receiver) = mpsc::channel();

        for i in 0..futures.len() {            
            let f = unsafe {Pin::new_unchecked(futures.pop().unwrap())};
            let task = Arc::new(Task {
                fut: Mutex::new(Some(f)),
                id: i,
                sender: sender.clone(),
            });
            // to trigger first poll
            sender.send(task).unwrap();
        }
        // drop the sender, and only have Task hold a sender clone, 
        // once all tasks finished, all sender are dropped, so the receiver will know we are done
        drop(sender);

        // --- Start Executor ---
        loop {
            // when receiver returns, it means the task is ready to advance further
            let t = match receiver.recv() {
                Ok(t) => t,
                Err(_) => break,
                
            };
            // --- create context for future ---
            // data for Waker will have both channel sender and the task
            // so that in `wake` call, we can send the task back to Executor for polling
            // Data has to be Arc so that as long as at least one waker exist, the data will be alive, so that the task and sender
            
            let rawwaker = RawWaker::new(
                Arc::into_raw(t.clone()) as *const (),
                &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
            );
            let waker = unsafe { Waker::from_raw(rawwaker) };
            let mut cx = Context::from_waker(&waker);

            // Need  mutex because we share the same underlying future across task references
            // We need to take the future out as mutable for calling `poll`, then
            // make sure the future stored back before exiting mutex guard
            let mut mtx = t.fut.lock().unwrap();
            // take the future out
            let mut f = mtx.take().unwrap();

            // Poll the future, note that we do not recreate the Waker and context here
            // because
            // 1. it is OK in our example because we only handle one Task and same underlying top future.
            // 2. we use the Waker's address as data for registering event, so should be kept until future completes
            //
            // Next step: to schedule multiple different tasks.
            // Need to recreate Waker and Context according to received Task, because of this,
            // if same Task is scheduled mulitple times, the context passed to underlying future will be different instances
            // so the address of Waker will not be unique, we can't use it as data for registering event
            let res = f.as_mut().poll(&mut cx);
            if let Poll::Ready(_) = res {
                println!("Task {} completed", t.id);
                continue;
                
            } else {
                // store the future back
                *mtx = Some(f);
                println!("Task {} is pending, waiting for next task.", t.id);
            }
            
        }
        println!("Exit runtime!");
    }
}

struct Task {
    fut: Mutex<Option<TopFuture>>,
    id: usize,
    sender: mpsc::Sender<Arc<Task>>,
}

fn clone_rw(p: *const ()) -> RawWaker {
    let data: Arc<Task> = unsafe { Arc::from_raw(p as *const Task) };

    // make sure increment reference count of the underlying source
    // clone increment ref count, into_raw consume the cloned and escape drop
    let p = Arc::into_raw(data.clone());
    // do not decrement ref count
    forget(data);

    // new RawWaker with data pointer to same Task
    RawWaker::new(
        p as *const (),
        // the `RawWakerVTable::new` is a magic `const` function can create a object with 'static lifetime
        &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
    )
}

fn wake_rw(p: *const ()) {
    let data: Arc<Task> = unsafe { Arc::from_raw(p as *const Task) };
    // this can be called from another thread
    let _mtx = data.fut.lock().unwrap();
    // sending task to executor
    data.sender.send(data.clone()).unwrap();
}

fn wake_by_ref_rw(p: *const ()) {
    let data: Arc<Task> = unsafe { Arc::from_raw(p as *const Task) };

    // this can be called from another thread
    {
        let _mtx = data.fut.lock().unwrap();
        // sending task to executor
        data.sender.send(data.clone()).unwrap();
    }
    forget(data);
}

fn drop_rw(p: *const ()) {
    unsafe { Arc::from_raw(p as *const Task) };
    // decrement reference count by auto drop
}
