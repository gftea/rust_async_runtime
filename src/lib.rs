use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    mem::forget,
    pin::Pin,
    process::Output,
    sync::{mpsc, Arc, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread,
    time::Duration,
};

// export net
pub mod net;

// for start reactor
mod reactor;
use reactor::epoll::Reactor;

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

pub struct Runtime;

impl Runtime {
    pub fn run<F: Future<Output = ()> + Send + 'static>(&self, f: F) {
        let (sender, receiver) = mpsc::channel();
        let f = Box::pin(f);

        let task = Arc::new(Task {
            fut: Mutex::new(Some(f)),
        });

        // hold the reactor
        let reactor = Reactor::new();
        // enter context
        reactor::epoll::enter(reactor);

        
        // create context
        let data = Arc::new(Resource {
            sender: sender.clone(),
            task: task.clone(),
        });
        let rawwaker = RawWaker::new(
            Arc::into_raw(data) as *const (),
            &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
        );
        let waker = unsafe { Waker::from_raw(rawwaker) };
        let mut cx = Context::from_waker(&waker);
        let p = &waker as *const Waker as usize;
        println!("waker address : {p:#x}");
        // start executor
        // trigger first poll
        sender.send(task.clone()).unwrap();

        loop {
            let t = receiver.recv().unwrap();

            // handle ready task
            // make sure we exit with the future stored back
            let mut mtx = t.fut.lock().unwrap();
            // take the future out
            let mut f = mtx.take().unwrap();
            let res = f.as_mut().poll(&mut cx);
            if let Poll::Ready(v) = res {
                break;
            }
            // store the future back
            *mtx = Some(f);
            // wake the task again in another thread
            // thread::spawn(move || {
            //     waker.wake();
            // });
            println!("top future pending, poll next");
            // thread::sleep(Duration::from_secs(1));
        }
    }
}
struct Resource {
    sender: mpsc::Sender<Arc<Task>>,
    task: Arc<Task>,
}
struct Task {
    fut: Mutex<Option<Pin<Box<dyn Future<Output = ()>>>>>,
}

fn clone_rw(p: *const ()) -> RawWaker {
    let data: Arc<Resource> = unsafe { Arc::from_raw(p as *const Resource) };

    // make sure increment reference count of the underlying source
    // clone increment ref count, into_raw consume the cloned and escape drop
    let p = Arc::into_raw(data.clone());
    // do not decrement ref count
    forget(data);

    // new RawWaker with data pointer to same resource
    RawWaker::new(
        p as *const (),
        // the `RawWakerVTable::new` is a magic `const` function can create a object with 'static lifetime
        &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
    )
}

fn wake_rw(p: *const ()) {
    let data: Arc<Resource> = unsafe { Arc::from_raw(p as *const Resource) };
    // this can be called from another thread
    let mtx = data.task.fut.lock().unwrap();

    data.sender.send(data.task.clone()).unwrap();
}

fn wake_by_ref_rw(p: *const ()) {
    let data: Arc<Resource> = unsafe { Arc::from_raw(p as *const Resource) };

    // todo wakeup
    {
        let _mtx = data.task.fut.lock().unwrap();
        // add to run queue
        data.sender.send(data.task.clone()).unwrap();
    }
    forget(data);
    //
}

fn drop_rw(p: *const ()) {
    unsafe { Arc::from_raw(p as *const Resource) };
    // decrement reference count by auto drop
}
