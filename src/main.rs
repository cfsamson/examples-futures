use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::task::Waker;
use std::task::{RawWaker, RawWakerVTable};

fn main() {
    let reactor = Reactor::new();
    let reactor = Arc::new(Mutex::new(reactor));
    let future1 = Task::new(reactor.clone(), 1);
    let future2 = Task::new(reactor.clone(), 2);

    // EXECUTOR - drive futures to completion
    let waker = waker_new(thread::current());
    let mut futures = vec![future1, future2];
    loop {
        let mut futures_to_remove = vec![];
        for (i, future) in futures.iter_mut().enumerate() {
            match future.poll(waker_into_waker(&waker)) {
                Async::Ready(val) => {
                    println!("Got {}", val);
                    futures_to_remove.push(i);
                }
                Async::Pending => (),
            };
        }
        for i in futures_to_remove {
            futures.remove(i);
        }
        if futures.is_empty() {
            let mut reactor_locked = reactor.lock().unwrap();
            reactor_locked.close();
            break;
        }

        thread::park();
    }
}

#[derive(Clone)]
struct MyWaker {
    thread: thread::Thread,
}

fn waker_new(thread: thread::Thread) -> MyWaker {
    MyWaker { thread }
}

fn waker_wake(s: &MyWaker) {
    s.thread.unpark();
}

fn waker_clone(s: &MyWaker) -> RawWaker {
    todo!()
}

const VTABLE: RawWakerVTable = unsafe {
    RawWakerVTable::new(
        |s| waker_clone(&*(s as *const MyWaker)),
        |s| waker_wake(&*(s as *const MyWaker)),
        |_| {},
        |_| {},
    )
};

fn waker_into_waker(s: &MyWaker) -> Waker {
    let self_data: *const MyWaker = s;
    let raw_waker = RawWaker::new(self_data as *const (), &VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

trait Fut {
    type Item;
    fn poll(&mut self, waker: Waker) -> Async<Self::Item>
    where
        Self::Item: std::fmt::Debug;
}

#[derive(Debug)]
enum Async<T: std::fmt::Debug> {
    Pending,
    Ready(T),
}

#[derive(Clone)]
pub struct Task {
    id: usize,
    reactor: Arc<Mutex<Reactor>>,
    data: u64,
}

impl Task {
    fn new(reactor: Arc<Mutex<Reactor>>, data: u64) -> Self {
        let mut react = reactor.lock().unwrap();
        let id = react.generate_id();
        drop(react);
        Task { id, reactor, data }
    }
}

impl Fut for Task {
    type Item = usize;
    fn poll(&mut self, waker: Waker) -> Async<Self::Item> {
        let mut r = self.reactor.lock().unwrap();
        if r.is_ready(self.id) {
            Async::Ready(self.id)
        } else {
            r.register(self.data, waker, self.id);
            Async::Pending
        }
    }
}

struct Reactor {
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    outstanding: Arc<AtomicUsize>,
    readylist: Arc<Mutex<Vec<usize>>>,
    id_generator: usize,
}

impl Reactor {
    fn new() -> Self {
        let (tx, rx) = channel::<Event>();
        let readylist = Arc::new(Mutex::new(vec![]));
        let rl_clone = readylist.clone();
        let outstanding = Arc::new(AtomicUsize::new(0));
        let outstanding_clone = outstanding.clone();
        let mut handles = vec![];
        let handle = thread::spawn(move || {
            // This simulates some I/O resource
            for event in rx {
                let outstanding = outstanding_clone.clone();
                let rl_clone = rl_clone.clone();
                match event {
                    Event::Close => break,
                    Event::Simple(waker, duration, id) => {
                        let event_handle = thread::spawn(move || {
                            thread::sleep(std::time::Duration::from_secs(duration));
                            outstanding.fetch_sub(1, Ordering::Relaxed);
                            let mut rl_clone_locked = rl_clone.lock().unwrap();
                            rl_clone_locked.push(id);
                            drop(rl_clone_locked);
                            waker.wake();
                        });

                        handles.push(event_handle);
                    }
                }
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });

        Reactor {
            readylist,
            dispatcher: tx,
            handle: Some(handle),
            outstanding,
            id_generator: 0,
        }
    }

    fn register(&mut self, duration: u64, waker: Waker, data: usize) {
        self.dispatcher
            .send(Event::Simple(waker, duration, data))
            .unwrap();
        self.outstanding.fetch_add(1, Ordering::Relaxed);
    }

    fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    fn is_ready(&self, id_to_check: usize) -> bool {
        let readylist_locked = self.readylist.lock().unwrap();
        readylist_locked.iter().any(|id| *id == id_to_check)
    }

    fn generate_id(&mut self) -> usize {
        self.id_generator += 1;
        self.id_generator
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
    }
}

enum Event {
    Close,
    Simple(Waker, u64, usize),
}
