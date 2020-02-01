use std::{
    future::Future, pin::Pin, sync::{mpsc::{channel, Sender}, Arc, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    thread::{self, JoinHandle}, time::{Duration, Instant}
};

fn main() {
    // This is just to make it easier for us to see when our Future was resolved
    let start = Instant::now();

    // Many runtimes create a glocal `reactor` we pass it as an argument
    let reactor = Reactor::new();
    // Since we'll share this between threads we wrap it in a 
    // atmically-refcounted- mutex.
    let reactor = Arc::new(Mutex::new(reactor));
    
    // We create two tasks:
    // - first parameter is the `reactor`
    // - the second is a timeout in seconds
    // - the third is an `id` to identify the task
    let future1 = Task::new(reactor.clone(), 2, 1);
    let future2 = Task::new(reactor.clone(), 1, 2);

    // an `async` block works the same way as an `async fn` in that it compiles
    // our code into a state machine, `yielding` at every `await` point.
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

    // Our executor can only run one and one future, this is pretty normal
    // though. You have a set of operations containing many futures that
    // ends up as a single future that drives them all to completion.
    let mainfut = async {
        fut1.await;
        fut2.await;
    };

    // This executor will block the main thread until the futures is resolved
    block_on(mainfut);
    // When we're done, we want to shut down our reactor thread so our program
    // ends nicely.
    reactor.lock().map(|mut r| r.close()).unwrap();
}

//// ============================ EXECUTOR ====================================

// Our executor takes any object which implements the `Future` trait
fn block_on<F: Future>(mut future: F) -> F::Output {
    // the first thing we do is to construct a `Waker` which we'll pass on to
    // the `reactor` so it can wake us up when an event is ready. 
    let mywaker = Arc::new(MyWaker{ thread: thread::current() }); 
    let waker = waker_into_waker(Arc::into_raw(mywaker));
    // The context struct is just a wrapper for a `Waker` object. Maybe in the
    // future this will do more, but right now it's just a wrapper.
    let mut cx = Context::from_waker(&waker);

    // We poll in a loop, but it's not a busy loop. It will only run when
    // an event occurs, or a thread has a "spurious wakeup" (an unexpected wakeup
    // that can happen for no good reason).
    let val = loop {
        // So, since we run this on one thread and run one future to completion
        // we can pin the `Future` to the stack. This is unsafe, but saves an
        // allocation. We could `Box::pin` it too if we wanted. This is however
        // safe since we don't move the `Future` here.
        let pinned = unsafe { Pin::new_unchecked(&mut future) };
        match Future::poll(pinned, &mut cx) {
            // when the Future is ready we're finished
            Poll::Ready(val) => break val,
            // If we get a `pending` future we just go to sleep...
            Poll::Pending => thread::park(),
        };
    };
    val
}

// ====================== FUTURE IMPLEMENTATION ==============================

// This is the definition of our `Waker`. We use a regular thread-handle here.
// It works but it's not a good solution. If one of our `Futures` holds a handle
// to our thread and takes it with it to a different thread the followinc could
// happen:
// 1. Our future calls `unpark` from a different thread
// 2. Our `executor` thinks that data is ready and wakes up and polls the future
// 3. The future is not ready yet but one nanosecond later the `Reactor` gets
// an event and calles `wake()` which also unparks our thread.
// 4. This could all happen before we go to sleep again since these processes
// run in parallel.
// 5. Our reactor has called `wake` but our thread is still sleeping since it was
// awake alredy at that point.
// 6. We're deadlocked and our program stops working
// There are many better soloutions, here are some:
// - Use `std::sync::CondVar`
// - Use [crossbeam::sync::Parker](https://docs.rs/crossbeam/0.7.3/crossbeam/sync/struct.Parker.html)
#[derive(Clone)]
struct MyWaker {
    thread: thread::Thread,
}

// This is the definition of our `Future`. It keeps all the information we
// need. This one holds a reference to our `reactor`, that's just to make
// this example as easy as possible. It doesn't need to hold a reference to
// the whole reactor, but it needs to be able to register itself with the
// reactor.
#[derive(Clone)]
pub struct Task {
    id: usize,
    reactor: Arc<Mutex<Reactor>>,
    data: u64,
    is_registered: bool,
}

// These are function definitions we'll use for our waker. Remember the
// "Trait Objects" chapter from the book.
fn mywaker_wake(s: &MyWaker) {
    let waker_ptr: *const MyWaker = s;
    let waker_arc = unsafe {Arc::from_raw(waker_ptr)};
    waker_arc.thread.unpark();
}

// Since we use an `Arc` cloning is just increasing the refcount on the smart
// pointer.
fn mywaker_clone(s: &MyWaker) -> RawWaker {
    let arc = unsafe { Arc::from_raw(s).clone() };
    std::mem::forget(arc.clone()); // increase ref count
    RawWaker::new(Arc::into_raw(arc) as *const (), &VTABLE)
}

// This is actually a "helper funtcion" to create a `Waker` vtable. In contrast
// to when we created a `Trait Object` from scratch we don't need to concern
// ourselves with the actual layout of the `vtable` and only provide a fixed
// set of functions
const VTABLE: RawWakerVTable = unsafe {
    RawWakerVTable::new(
        |s| mywaker_clone(&*(s as *const MyWaker)),     // clone
        |s| mywaker_wake(&*(s as *const MyWaker)),      // wake
        |s| mywaker_wake(*(s as *const &MyWaker)),      // wake by ref
        |s| drop(Arc::from_raw(s as *const MyWaker)),   // decrease refcount
    )
};

// Instead of implementing this on the `MyWaker` oject in `impl Mywaker...` we
// just use this pattern instead since it saves us some lines of code.
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

// This is our `Future` implementation
impl Future for Task {
    // The output for this kind of `leaf future` is just an `usize`. For other
    // futures this could be something more interesting like a byte stream.
    type Output = usize;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();
        // we check with the `Reactor` if this future is in its "readylist"
        if r.is_ready(self.id) {
            // if it is, we return the data. In this case it's just the ID of
            // the task. 
            Poll::Ready(self.id)
        } else if self.is_registered {
            // If the future is registered alredy, we just return `Pending`
            Poll::Pending
        } else {
            // If we get here, it must be the first time this `Future` is polled
            // so we register a task with our `reactor`
            r.register(self.data, cx.waker().clone(), self.id);
            // oh, we have to drop the lock on our `Mutex` here because we can't
            // have a shared and exclusive borrow at the same time
            drop(r);
            self.is_registered = true;
            Poll::Pending
        }
    }
}

// =============================== REACTOR ===================================

// This is a "fake" reactor. It does no real I/O, but that also makes our
// code possible to run in the book and in the playground
struct Reactor {
    // we need some way of registering a Task with the reactor. Normally this
    // would be an "interest" in an I/O event
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    // This is a list of tasks that are ready, which means they should be polled
    // for data.
    readylist: Arc<Mutex<Vec<usize>>>,
}

// We just have two kind of events. A timeout event, a "timeout" event called
// `Simple` and a `Close` event to close down our reactor.
#[derive(Debug)]
enum Event {
    Close,
    Timeout(Waker, u64, usize),
}

impl Reactor {
    fn new() -> Self {
        // The way we register new events with our reactor is using a regular
        // channel
        let (tx, rx) = channel::<Event>();
        let readylist = Arc::new(Mutex::new(vec![]));
        let rl_clone = readylist.clone();

        // This `Vec` will hold handles to all threads we spawn so we can
        // join them later on and finish our programm in a good manner
        let mut handles = vec![];
        // This will be the "Reactor thread"
        let handle = thread::spawn(move || {
            // This simulates some I/O resource
            for event in rx {
                let rl_clone = rl_clone.clone();
                match event {
                    // If we get a close event we break out of the loop we're in
                    Event::Close => break,
                    Event::Timeout(waker, duration, id) => {

                        // When we get an event we simply spawn a new thread...
                        let event_handle = thread::spawn(move || {
                            //... which will just sleep for the number of seconds
                            // we provided when creating the `Task`.
                            thread::sleep(Duration::from_secs(duration));
                            // When it's done sleeping we put the ID of this task
                            // on the "readylist"
                            rl_clone.lock().map(|mut rl| rl.push(id)).unwrap();
                            // Then we call `wake` which will wake up our
                            // executor and start polling the futures
                            waker.wake();
                        });

                        handles.push(event_handle);
                    }
                }
            }

            // When we exit the Reactor we first join all the handles on
            // the child threads we've spawned so we catch any panics and
            // release all resources.
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
        // registering an event is as simple as sending an `Event` through
        // the channel.
        self.dispatcher
            .send(Event::Timeout(waker, duration, data))
            .unwrap();
    }

    fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    // We need a way to check if any event's are ready. This will simply
    // look through the "readylist" for an event macthing the ID we want to
    // check for.
    fn is_ready(&self, id_to_check: usize) -> bool {
        self.readylist
            .lock()
            .map(|rl| rl.iter().any(|id| *id == id_to_check))
            .unwrap()
    }
}

// When our `Reactor` is dropped we join the reactor thread with the thread
// owning our `Reactor` so we catch any panics and release all resources.
// It's not needed for this to work, but it really is a best practice to join
// all threads you spawn.
impl Drop for Reactor {
    fn drop(&mut self) {
        self.handle.take().map(|h| h.join().unwrap()).unwrap();
    }
}