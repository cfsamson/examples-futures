fn main() {
    let mut gen = GeneratorA::new();

    
    match Pin::new(&mut gen).as_mut().resume() {
        GeneratorState::Yielded(n) => {
            println!("Got value {}", n);
        }
        _ => (),
    };

    match Pin::new(&mut gen).as_mut().resume() {
        GeneratorState::Complete(()) => (),
        _ => (),
    };

}

// let b = || {
//         let a = String::from("Hello");---|
//         println!("{}", &a):              |
//         yield &a;                     |-- borrow across yield point  
//         println!("{} world!", &a); ------|
//     };
//
// let gen = b(4);
// let val = gen.resume();
// println!("Got value {}", val);
// gen.resume();

/// Will get compiled into

// If you've ever wondered why the parameters are called Y and R the naming from
// the original rfc most likely holds the answer
enum GeneratorState<Y, R> {
    // originally called `CoResult`
    Yielded(Y),  // originally called `Yield(Y)`
    Complete(R), // originally called `Return(R)`
}

enum State {
    Enter,
    Yield1,
    Exit,
}

use std::ptr;
use std::pin::Pin;
struct GeneratorA {
    local_a: Vec<char>,
    local_b: ptr::NonNull<*const [char]>,
    state: State,
}

impl GeneratorA {
    fn new() -> Self {
        GeneratorA {
            local_a: vec![],
            local_b: ptr::NonNull::dangling(),
            state: State::Enter,
        }
    }
}

trait Generator {
    type Yield; 
    type Return;
    fn resume(self: Pin<&mut Self>) -> GeneratorState<Self::Yield, Self::Return>;
}

impl Generator for GeneratorA {
    type Yield = String;
    type Return = ();
    fn resume(self: Pin<&mut Self>) -> GeneratorState<Self::Yield, Self::Return> {
        let this = self.get_mut();
        match this.state {
            State::Enter => {
                let local_a = vec!['H', 'e', 'l','l','o',' ', 'w','o','r','d'];
                this.local_a = local_a;
                let mut local_b: *const [char] = &this.local_a[0..5];
                this.local_b = ptr::NonNull::from(&mut local_b);
                let local_b = unsafe { &**this.local_b.as_ref() };
                let out: String = local_b.iter().collect();
                this.state = State::Yield1;
                GeneratorState::Yielded(out)
            }

            State::Yield1 => {
                let local_b = unsafe { &**this.local_b.as_ref() };
                println!("Hello {}", local_b.iter().collect::<String>());
                this.state = State::Exit;
                GeneratorState::Complete(())
            }
            State::Exit => {
                panic!("Can't advance an exited generator!")
            }
        }
    }
}
