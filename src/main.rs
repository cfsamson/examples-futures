use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use std::task::Waker;
use std::task::{RawWaker, RawWakerVTable};

fn main() {
    let rl = Arc::new(Mutex::new(vec![]));
    let mut reactor = Reactor::new();
    let waker = waker_new(1, thread::current(), rl.clone());
    reactor.register(3, waker_into_waker(&waker));

    let waker = waker_new(2, thread::current(), rl.clone());
    reactor.register(2, waker_into_waker(&waker));
    reactor.close();

    loop {
        let mut rl_locked = rl.lock().unwrap();
        while let Some(event) = rl_locked.pop() {
            println!("Event {} just happened.", event);
        }
        drop(rl_locked);
        if reactor.outstanding() == 0 {
            break;
        }

        thread::park();
    }
}

#[derive(Clone)]
struct MyWaker {
    id: usize,
    thread: thread::Thread,
    readylist: Arc<Mutex<Vec<usize>>>,
}

fn waker_new(id: usize, thread: thread::Thread, readylist: Arc<Mutex<Vec<usize>>>) -> MyWaker {
    MyWaker {
        id,
        thread,
        readylist,
    }
}

fn waker_wake(s: &MyWaker) {
    let mut readylist = s.readylist.lock().unwrap();
    readylist.push(s.id);
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
    let waker = unsafe { Waker::from_raw(raw_waker) };
    waker
}


#[derive(Clone)]
pub struct Task {
    id: usize,
}

struct Reactor {
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    outstanding: Arc<AtomicUsize>,
}

impl Reactor {
    fn new() -> Self {
        let (tx, rx) = channel::<Event>();
        let outstanding = Arc::new(AtomicUsize::new(0));
        let outstanding_clone = outstanding.clone();
        let mut handles = vec![];
        let handle = thread::spawn(move || {
            // This simulates some I/O resource
            for event in rx {
                let outstanding = outstanding_clone.clone();
                match event {
                    Event::Close => break,
                    Event::Simple(waker, sleep) => {
                        let event_handle = thread::spawn(move || {
                            thread::sleep(std::time::Duration::from_secs(sleep));
                            outstanding.fetch_sub(1, Ordering::Relaxed);
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
            dispatcher: tx,
            handle: Some(handle),
            outstanding,
        }
    }

    fn register(&mut self, duration: u64, waker: Waker) {
        self.dispatcher
            .send(Event::Simple(waker, duration))
            .unwrap();
        self.outstanding.fetch_add(1, Ordering::Relaxed);
    }

    fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    fn outstanding(&self) -> usize {
        self.outstanding.load(Ordering::Relaxed)
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
    Simple(Waker, u64),
}
