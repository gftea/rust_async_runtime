use std::{cell::RefCell, io::ErrorKind, task::Waker, thread};

use crate::syscall;

thread_local! {static REACTOR_CONTEXT: RefCell<Option<Registry>> = RefCell::new(None);}

/// before polling a future, we need to call this for
/// 1. spawn the reactor thread (different from calling thread) for listening event
/// 2. for the calling thread, set its REACTOR_CONTEXT with needed info for registering event to the spawned reactor
///
/// NOTE: we make sure the future is polled within the same calling thread,
///       so it can get the spawned reactor context and register the event interest.
pub fn enter() {
    REACTOR_CONTEXT.with(|r| {
        *r.borrow_mut() = Some(Reactor::spawn());
        println!(
            "enter in {:?} | {}:{} |",
            thread::current().id(),
            file!(),
            line!(),
        );
    });
}

/// Get the reactor's registery for registering event
/// To be used in `Future::poll`
pub fn get_registery() -> Option<Registry> {
    REACTOR_CONTEXT.with(|r| {
        let registry = r.borrow().expect("Reactor Context don't exist!");
        println!(
            "get registery in {:?} | {}:{} | ",
            thread::current().id(),
            file!(),
            line!(),
        );
        Some(registry)
    })
}

#[derive(Copy, Clone)]
pub struct Reactor {
    epoll: i32,
}

/// We only need epoll id in order to register event to the correct Reactor
#[derive(Copy, Clone)]
pub struct Registry {
    epoll: i32,
}
impl Reactor {
    fn spawn() -> Registry {
        let epoll = syscall!(epoll_create(1)).unwrap();
        thread::spawn(move || {
            println!(
                "Reactor spawned in: {:?} | {}:{} | ",
                thread::current().id(),
                file!(),
                line!(),
            );
            let mut events: Vec<libc::epoll_event> = Vec::with_capacity(10);
            loop {
                // epoll_wait can be called before registering interests with epoll_ctl
                let res =
                    syscall!(epoll_wait(epoll, events.as_mut_ptr(), 10, -1)).unwrap() as usize;
                if res > 0 {
                    unsafe {
                        events.set_len(res);
                    }
                    for i in 0..res {
                        // destruct the ready event
                        let libc::epoll_event {
                            events: event_mask,
                            u64: data,
                        } = events[i];
                        println!(
                            "Received ready events in {:?}, index: {i} event mask: {event_mask}, data: {data:#x}. | {}:{} | ",
                            thread::current().id(),
                            file!(),
                            line!(),
                        );
                        // when we register event, we provide the waker's address as data
                        // This is safe in our example
                        // 1. only single waker for the task
                        let waker = unsafe { &*(data as *const () as *const Waker) };
                        waker.clone().wake();
                    }
                }
            }
        });
        Registry { epoll }
    }
}

impl Registry {
    pub fn register(&self, fd: i32, event: i32, data: u64) {
        let mut event = libc::epoll_event {
            events: (0i32 | event) as u32,
            u64: data,
        };
        match syscall!(epoll_ctl(self.epoll, libc::EPOLL_CTL_ADD, fd, &mut event)) {
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => println!(
                "registration already exist: {:?} | {}:{} | ",
                thread::current().id(),
                file!(),
                line!(),
            ),
            Err(_) => panic!("Reactor registration error!"),
        }
    }
}
