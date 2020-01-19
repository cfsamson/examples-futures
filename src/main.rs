use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use std::task::Waker;
use std::task::{RawWaker, RawWakerVTable};

fn main() {
    let rl = Arc::new(Mutex::new(vec![]));
    let mut reactor = Reactor::new();
    let waker = MyWaker::new(1, thread::current(), rl.clone());
    reactor.register(2, waker);

    let waker = MyWaker::new(2, thread::current(), rl.clone());
    reactor.register(1, waker);
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

struct MyWaker {
    id: usize,
    thread: thread::Thread,
    readylist: Arc<Mutex<Vec<usize>>>,
}

impl MyWaker {
    fn new(id: usize, thread: thread::Thread, readylist: Arc<Mutex<Vec<usize>>>) -> Self {
        MyWaker {
            id,
            thread,
            readylist,
        }
    }
    fn wake(&self) {
        let mut readylist = self.readylist.lock().unwrap();
        readylist.push(self.id);
        self.thread.unpark();  
    }

    fn into_waker(&self) -> Waker {
        let vtable = RawWakerVTable::new(|_| {}, Self::wake as Fn(MyWaker), |_| {}, |_| {});
        let raw_waker = RawWaker::new(&self, vtable);
        let waker = Waker::from_raw(raw_waker);
    }
}

trait Fut {
    type Item;
    type Error;
    fn poll(&mut self) -> Result<Aync<Self::Item>, Self::Error>;
}

enum Aync<T> {
    NotReady,
    Ready(T),
}

#[derive(Clone)]
pub struct Task {
    id: usize,
    unpark: SomeThingThatWakesUpTask,
}

impl Task {
    fn unpark(&self) {
        self.unpark.notify();
    }
}

#[derive(Clone)]
struct SomeThingThatWakesUpTask {}

impl SomeThingThatWakesUpTask {
    pub fn notify(&self) {
        todo!()
    }
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
