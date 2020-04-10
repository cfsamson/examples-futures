use std::{
    future::Future, pin::Pin, sync::{mpsc::{channel, Sender}, Arc, Mutex},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker}, mem,
    thread::{self, JoinHandle}, time::{Duration, Instant}, collections::HashMap,
};

fn main() {
    // This is just to make it easier for us to see when our Future was resolved
    let start = Instant::now();

    // Many runtimes create a glocal `reactor` we pass it as an argument
    let reactor = Reactor::new();
    
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
    reactor: Arc<Mutex<Box<Reactor>>>,
    data: u64,
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
    fn new(reactor: Arc<Mutex<Box<Reactor>>>, data: u64, id: usize) -> Self {
        Task { id, reactor, data }
    }
}

impl Future for Task {
    type Output = usize;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();

        // First we check if the task is marked as ready
        if r.is_ready(self.id) {
            *r.tasks.get_mut(&self.id).unwrap() = TaskState::Finished;
            Poll::Ready(self.id)
        
        // If it isn't we check the Reactors map over id's we have registered
        // and see if it's there
        } else if r.tasks.contains_key(&self.id) {
            // This is important. The docs says that on multiple calls to poll,
            // only the Waker from the Context passed to the most recent call
            // should be scheduled to receive a wakeup. That's why we insert
            // this waker into the map (which will return the old one which will
            // get dropped) before we return `Pending`.
            r.tasks.insert(self.id, TaskState::NotReady(cx.waker().clone()));
            Poll::Pending
        } else {
            // If it's not ready, and not in the map it's a new task so we
            // register that with the Reactor.
            r.register(self.data, cx.waker().clone(), self.id);
            Poll::Pending
        }
    }
}


// =============================== REACTOR ===================================

// The different states a task can have in this Reactor
enum TaskState {
    Ready,
    NotReady(Waker),
    Finished,
}

// This is a "fake" reactor. It does no real I/O, but that also makes our
// code possible to run in the book and in the playground
struct Reactor {

    // we need some way of registering a Task with the reactor. Normally this
    // would be an "interest" in an I/O event
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,

    // This is a list of tasks
    tasks: HashMap<usize, TaskState>,
}

// This represents the Events we can send to our reactor thread. In this
// example it's only a Timeout or a Close event.
#[derive(Debug)]
enum Event {
    Close,
    Timeout(u64, usize),
}

impl Reactor {

    // We choose to return an atomic reference counted, mutex protected, heap
    // allocated `Reactor`. Just to make it easy to explain... No, the reason
    // we do this is:
    //
    // 1. We know that only thread-safe reactors will be created.
    // 2. By heap allocating it we can obtain a reference to a stable address
    // that's not dependent on the stack frame of the function that called `new`
    fn new() -> Arc<Mutex<Box<Self>>> {
        let (tx, rx) = channel::<Event>();
        let reactor = Arc::new(Mutex::new(Box::new(Reactor {
            dispatcher: tx,
            handle: None,
            tasks: HashMap::new(),
        })));
        
        // Notice that we'll need to use `weak` reference here. If we don't,
        // our `Reactor` will not get `dropped` when our main thread is finished
        // since we're holding internal references to it.

        // Since we're collecting all `JoinHandles` from the threads we spawn
        // and make sure to join them we know that `Reactor` will be alive
        // longer than any reference held by the threads we spawn here.
        let reactor_clone = Arc::downgrade(&reactor);

        // This will be our Reactor-thread. The Reactor-thread will in our case
        // just spawn new threads which will serve as timers for us.
        let handle = thread::spawn(move || {
            let mut handles = vec![];

            // This simulates some I/O resource
            for event in rx {
                println!("REACTOR: {:?}", event);
                let reactor = reactor_clone.clone();
                match event {
                    Event::Close => break,
                    Event::Timeout(duration, id) => {

                        // We spawn a new thread that will serve as a timer
                        // and will call `wake` on the correct `Waker` once
                        // it's done.
                        let event_handle = thread::spawn(move || {
                            thread::sleep(Duration::from_secs(duration));
                            let reactor = reactor.upgrade().unwrap();
                            reactor.lock().map(|mut r| r.wake(id)).unwrap();
                        });
                        handles.push(event_handle);
                    }
                }
            }

            // This is important for us since we need to know that these
            // threads don't live longer than our Reactor-thread. Our
            // Reactor-thread will be joined when `Reactor` gets dropped.
            handles.into_iter().for_each(|handle| handle.join().unwrap());
        });
        reactor.lock().map(|mut r| r.handle = Some(handle)).unwrap();
        reactor
    }

    // The wake function will call wake on the waker for the task with the
    // corresponding id.
    fn wake(&mut self, id: usize) {
        self.tasks.get_mut(&id).map(|state| {

            // No matter what state the task was in we can safely set it
            // to ready at this point. This lets us get ownership over the
            // the data that was there before we replaced it.
            match mem::replace(state, TaskState::Ready) {
                TaskState::NotReady(waker) => waker.wake(),
                TaskState::Finished => panic!("Called 'wake' twice on task: {}", id),
                _ => unreachable!()
            }
        }).unwrap();
    }

    // Register a new task with the reactor. In this particular example
    // we panic if a task with the same id get's registered twice 
    fn register(&mut self, duration: u64, waker: Waker, id: usize) {
        if self.tasks.insert(id, TaskState::NotReady(waker)).is_some() {
            panic!("Tried to insert a task with id: '{}', twice!", id);
        }
        self.dispatcher.send(Event::Timeout(duration, id)).unwrap();
    }

    // We send a close event to the reactor so it closes down our reactor-thread
    fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    // We simply checks if a task with this id is in the state `TaskState::Ready`
    fn is_ready(&self, id: usize) -> bool {
        self.tasks.get(&id).map(|state| match state {
            TaskState::Ready => true,
            _ => false,
        }).unwrap_or(false)
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.handle.take().map(|h| h.join().unwrap()).unwrap();
    }
}