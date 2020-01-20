use std::sync::{
    mpsc::{channel, Sender},
    Arc, Mutex,
};
use std::task::{RawWaker, RawWakerVTable, Waker, Context, Poll};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::future::Future;
use std::pin::Pin;

fn main() {
    let reactor = Reactor::new();
    let reactor = Arc::new(Mutex::new(reactor));
    let future1 = Task::new(reactor.clone(), 1);
    let future2 = Task::new(reactor.clone(), 2);
    let futures = vec![future1, future2];
    block_on_all(futures, reactor);
}

//// ===== EXECUTOR =====
fn block_on_all(mut futures: Vec<Task>, reactor: Arc<Mutex<Reactor>>) {
    let waker = waker_new(thread::current());
    loop {
        let mut futures_to_remove = vec![];
        for (i, future) in futures.iter_mut().enumerate() {
            let pinned = Pin::new(future);
            let waker = waker_into_waker(&waker);
            let mut cx = Context::from_waker(&waker);
            match Task::poll(pinned, &mut cx) {
                Poll::Ready(val) => {
                    println!("Got {}", val);
                    futures_to_remove.push(i);
                }
                Poll::Pending => (),
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

// ===== FUTURE IMPLEMENTATION =====
#[derive(Clone)]
struct MyWaker {
    thread: thread::Thread,
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

fn waker_new(thread: thread::Thread) -> MyWaker {
    MyWaker { thread }
}

fn waker_wake(s: &MyWaker) {
    s.thread.unpark();
}

fn waker_clone(s: &MyWaker) -> RawWaker {
    let s: *const MyWaker = s;
    RawWaker::new(s as *const (), &VTABLE)
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

impl Task {
    fn new(reactor: Arc<Mutex<Reactor>>, data: u64) -> Self {
        let id = reactor.lock().map(|mut r| r.generate_id()).unwrap();
        Task { id, reactor, data }
    }
    
}

impl Future for Task {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();
        if r.is_ready(self.id) {
            Poll::Ready(self.id)
        } else {
            r.register(self.data, cx.waker().clone(), self.id);
            Poll::Pending
        }
    }
}

// ===== REACTOR =====
struct Reactor {
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    readylist: Arc<Mutex<Vec<usize>>>,
    id_generator: usize,
}

enum Event {
    Close,
    Simple(Waker, u64, usize),
}

impl Reactor {
    fn new() -> Self {
        let (tx, rx) = channel::<Event>();
        let readylist = Arc::new(Mutex::new(vec![]));
        let rl_clone = readylist.clone();
        let mut handles = vec![];
        let handle = thread::spawn(move || {
            // This simulates some I/O resource
            for event in rx {
                let rl_clone = rl_clone.clone();
                match event {
                    Event::Close => break,
                    Event::Simple(waker, duration, id) => {
                        let event_handle = thread::spawn(move || {
                            thread::sleep(Duration::from_secs(duration));
                            rl_clone.lock().map(|mut rl| rl.push(id)).unwrap();
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
            id_generator: 0,
        }
    }

    fn register(&mut self, duration: u64, waker: Waker, data: usize) {
        self.dispatcher
            .send(Event::Simple(waker, duration, data))
            .unwrap();
    }

    fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    fn is_ready(&self, id_to_check: usize) -> bool {
        self.readylist
            .lock()
            .map(|rl| rl.iter().any(|id| *id == id_to_check))
            .unwrap()
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