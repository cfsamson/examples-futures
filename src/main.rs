use std::future::Future;
use std::pin::Pin;
use std::sync::{
    mpsc::{channel, Sender},
    Arc, Mutex,
};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use async_std::task;

fn main() {
    let reactor = Reactor::new();
    let reactor = Arc::new(Mutex::new(reactor));
    let future1 = Task::new(reactor.clone(), 3, 1);
    let future2 = Task::new(reactor.clone(), 2, 2);

    let fut1 = async {
        println!("Future got: {}", future1.await);
    };

    let fut2 = async {
        println!("Future got: {}", future2.await);
    };

    let mainfut = async {
        fut1.await;
        fut2.await;
    };

    task::block_on(mainfut);
    reactor.lock().map(|mut r| r.close()).unwrap();
}

//// ===== EXECUTOR =====
fn block_on<F: Future>(mut future: F) -> F::Output {
    let waker = waker_new(thread::current());
    let waker = waker_into_waker(&waker);
    let mut cx = Context::from_waker(&waker);
    let val = loop {
        let pinned = unsafe { Pin::new_unchecked(&mut future) };
        match Future::poll(pinned, &mut cx) {
            Poll::Ready(val) => break val,
            Poll::Pending => thread::park(),
        };
    };
    val
}

// ===== FUTURE IMPLEMENTATION =====
#[derive(Clone)]
struct MyWaker {
    thread: thread::Thread,
}

#[derive(Clone)]
pub struct Task {
    id: usize,
    reactor: Arc<Mutex<Reactor>>,
    data: u64,
    is_registered: bool,
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
    fn new(reactor: Arc<Mutex<Reactor>>, data: u64, id: usize) -> Self {
        Task {
            id,
            reactor,
            data,
            is_registered: false,
        }
    }
}

impl Future for Task {
    type Output = usize;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();
        if r.is_ready(self.id) {
            Poll::Ready(self.id)
        } else if self.is_registered {
            Poll::Pending
        } else {
            r.register(self.data, cx.waker().clone(), self.id);
            drop(r);
            self.is_registered = true;
            Poll::Pending
        }
    }
}

// ===== REACTOR =====
struct Reactor {
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    readylist: Arc<Mutex<Vec<usize>>>,
}
#[derive(Debug)]
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
}

impl Drop for Reactor {
    fn drop(&mut self) {
        let handle = self.handle.take().unwrap();
        handle.join().unwrap();
    }
}