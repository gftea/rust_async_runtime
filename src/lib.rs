use std::{
    future::Future,
    mem::forget,
    net::TcpStream,
    sync::Arc,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread, io::{Write, Read}, os::unix::prelude::AsRawFd,
};

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
    pub fn run<F: Future>(&self, mut f: F) {
        // create context
        let data = Arc::new(Task);

        let waker = RawWaker::new(
            Arc::into_raw(data) as *const (),
            &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
        );
        let waker = unsafe { Waker::from_raw(waker) };
        let mut cx = Context::from_waker(&waker);

        // pin to heap
        let mut f = Box::pin(f);

        // start reactor
        let reactor = Reactor::new();
        reactor.start();

        // start executor
        loop {
            let res = f.as_mut().poll(&mut cx);
            if let Poll::Ready(v) = res {
                break;
            }
        }
    }
}

struct AsyncTcpStream {
    stream: TcpStream,
    epoll_fd: i32,
}

impl AsyncTcpStream {
    fn connect(addr: &str) -> TcpStream {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.stream.write(buf)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut event = libc::epoll_event {
            events: (0i32 | libc::EPOLLIN) as u32,
            u64: 0xfeedbeef,
        };

        syscall!(epoll_ctl(
            self.epoll_fd,
            libc::EPOLL_CTL_ADD,
            self.stream.as_raw_fd(),
            &mut event
        ))
        .unwrap();
    }
}
impl Future for AsyncTcpStream {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        
    }
}

struct Reactor {
    epoll_fd: i32,
}

impl Reactor {
    fn new() -> Reactor {
        let epoll_fd = syscall!(epoll_create(1)).unwrap();

        Reactor { epoll_fd }
    }

    fn register(&self, token: u64, fd: i32) {
        let mut event = libc::epoll_event {
            events: (0i32 | libc::EPOLLIN) as u32,
            u64: token,
        };

        syscall!(epoll_ctl(
            self.epoll_fd,
            libc::EPOLL_CTL_ADD,
            fd,
            &mut event
        ))
        .unwrap();
    }

    fn start(&self) {
        let mut events: Vec<libc::epoll_event> = Vec::with_capacity(10);
        let epoll_fd = self.epoll_fd;
        thread::spawn(move || {
            // blocking
            loop {
                let res =
                    syscall!(epoll_wait(epoll_fd, events.as_mut_ptr(), 10, 2000)).unwrap() as usize;
                if res > 0 {
                    unsafe {
                        events.set_len(res);
                    }
                    for i in 0..res {
                        let ready_event = events[i];
                        let event_mask = ready_event.events;
                        let token = ready_event.u64;
                        println!("event mask: {}, token: {:#x}", event_mask, token);
                    }
                } else {
                    println!("timeout, return {}", res);
                    break;
                }
            }
        });
    }
}

fn clone_rw(p: *const ()) -> RawWaker {
    let data: Arc<Task> = unsafe { Arc::from_raw(p as *const Task) };

    // make sure increment reference count of the underlying source
    // clone increment ref count, into_raw consume the cloned and escape drop
    let p = Arc::into_raw(data.clone());
    // do not decrement ref count
    forget(data);

    // new RawWaker with data pointer to same resource
    RawWaker::new(
        p as *const (),
        &RawWakerVTable::new(clone_rw, wake_rw, wake_by_ref_rw, drop_rw),
    )
}

fn wake_rw(p: *const ()) {
    let data: Arc<Task> = unsafe { Arc::from_raw(p as *const Task) };
    // todo wakeup
}

fn wake_by_ref_rw(p: *const ()) {
    let data: Arc<Task> = unsafe { Arc::from_raw(p as *const Task) };
    // todo wakeup
    forget(data);
}

fn drop_rw(p: *const ()) {
    unsafe { Arc::from_raw(p as *const Task) };
    // decrement reference count by auto drop
}

struct Executor;
struct Task;
