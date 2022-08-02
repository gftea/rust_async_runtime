use std::{
    cell::RefCell,
    io::{Empty, ErrorKind},
    sync::{mpsc, Arc},
    task::Waker, thread,
};

use crate::syscall;

thread_local! {static REACTOR_CONTEXT: RefCell<Option<Reactor>> = RefCell::new(None);}

pub fn enter(r: Reactor) {
    REACTOR_CONTEXT.with(|r| {
        *r.borrow_mut() = Some(Reactor::new());
        let tid = thread::current().id();
        println!("enter thread {tid:?}");
    });
}
pub fn get_reactor() -> Option<Reactor> {
    REACTOR_CONTEXT.with(|r| {
        let reactor = r.borrow();
        let tid = thread::current().id();
        println!("get reactor thread {tid:?}");
        reactor.clone()
    })
}


#[derive(Copy, Clone)]
pub struct Reactor {
    epoll: i32,
}

impl Reactor {
    pub fn new() -> Self {
        let epoll = syscall!(epoll_create(1)).unwrap();
        thread::spawn(move || {
            let mut events: Vec<libc::epoll_event> = Vec::with_capacity(10);
            loop {
                let res =
                    syscall!(epoll_wait(epoll, events.as_mut_ptr(), 10, -1)).unwrap() as usize;
                if res > 0 {
                    unsafe {
                        events.set_len(res);
                    }
                    for i in 0..res {
                        let libc::epoll_event {
                            events: event_mask,
                            u64: data,
                        } = events[i];
                        let thread_id = thread::current().id();
                        println!("{thread_id:?} ready index: {i} event mask: {}, data: {:#x}", event_mask, data);
                        let waker = unsafe { &*(data as *const () as *const Waker)};
                        waker.clone().wake();
                    }
                }
            }
        });
        Reactor { epoll }
    }

    /// todo: abstract the data type
    pub fn register(&self, fd: i32, event: i32, data: u64) {
        let mut event = libc::epoll_event {
            events: (0i32 | event) as u32,
            u64: data,
        };
        match syscall!(epoll_ctl(self.epoll, libc::EPOLL_CTL_ADD, fd, &mut event)) {
            Ok(_) => (),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
            Err(_) => panic!("reactor registration erro"),
        }
    }

  
}
