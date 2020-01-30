use std::{
    future::Future, pin::Pin, sync::{mpsc::{channel, Sender}, Arc, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread::{self, JoinHandle}, time::{Duration, Instant}
};

fn main() {
    let start = Instant::now();

    // Many runtimes create a glocal `reactor` we pass it as an argument
    let reactor = Reactor::new();
    let reactor = Arc::new(Mutex::new(reactor));
    
    let future1 = Task::new(reactor.clone(), 2, 1);
    let future2 = Task::new(reactor.clone(), 1, 2);

    let fut1 = async {
        let val = future1.await;
        let dur = (Instant::now() - start).as_secs_f32();
        println!("Future got {} at time: {:.2}.", val, dur);
    };

    let fut2 = async {
        let val = future2.await;
        let dur = (Instant::now() - start).as_secs_f32();
        println!("Future got {} at time: {:.2}.", val, dur);
    };

    let mainfut = async {
        fut1.await;
        fut2.await;
    };

    block_on(mainfut);
    reactor.lock().map(|mut r| r.close()).unwrap();
}

//// ============================ EXECUTOR ====================================
fn block_on<F: Future>(mut future: F) -> F::Output {
    let mywaker = Arc::new(MyWaker{ thread: thread::current() }); 
    let waker = waker_into_waker(Arc::into_raw(mywaker));
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

// ====================== FUTURE IMPLEMENTATION ==============================
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

fn mywaker_wake(s: &MyWaker) {
    let waker_ptr: *const MyWaker = s;
    let waker_arc = unsafe {Arc::from_raw(waker_ptr)};
    waker_arc.thread.unpark();
}

fn mywaker_clone(s: &MyWaker) -> RawWaker {
    let arc = unsafe { Arc::from_raw(s).clone() };
    std::mem::forget(arc.clone()); // increase ref count
    RawWaker::new(Arc::into_raw(arc) as *const (), &VTABLE)
}

const VTABLE: RawWakerVTable = unsafe {
    RawWakerVTable::new(
        |s| mywaker_clone(&*(s as *const MyWaker)),     // clone
        |s| mywaker_wake(&*(s as *const MyWaker)),      // wake
        |s| mywaker_wake(*(s as *const &MyWaker)),      // wake by ref
        |s| drop(Arc::from_raw(s as *const MyWaker)),   // decrease refcount
    )
};

fn waker_into_waker(s: *const MyWaker) -> Waker {
    let raw_waker = RawWaker::new(s as *const (), &VTABLE);
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

// =============================== REACTOR ===================================
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
        self.handle.take().map(|h| h.join().unwrap()).unwrap();
    }
}