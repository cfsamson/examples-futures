fn main() {
    let mut gen = GeneratorA::new();

    
    match gen.resume() {
        GeneratorState::Yielded(n) => {
            println!("Got value {}", n);
        }
        _ => (),
    }

    match gen.resume() {
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
use std::marker::PhantomData;
use std::ptr;
struct GeneratorA<'a> {
    local_a: Vec<char>,
    local_b: ptr::NonNull<&'a [char]>,
    state: State,
    _marker: PhantomData<&'a [char]>,
}

impl<'a> GeneratorA<'a> {
    fn new() -> Self {
        GeneratorA {
            local_a: vec![],
            local_b: ptr::NonNull::dangling(),
            state: State::Enter,
            _marker: PhantomData,
        }
    }
}

trait Generator<'a> {
    type Yield; 
    type Return;
    fn resume(&'a mut self) -> GeneratorState<Self::Yield, Self::Return>;
}

impl<'a> Generator<'a> for GeneratorA<'a> {
    type Yield = &'a [char];
    type Return = ();
    fn resume(&'a mut self) -> GeneratorState<Self::Yield, Self::Return> {
        match self.state {
            State::Enter => {
                let local_a = vec!['H', 'e', 'l','l','o',' ', 'w','o','r','d'];
                self.local_a = local_a;
                self.local_b = ptr::NonNull::new(&mut &self.local_a[0..5]).unwrap();
                self.state = State::Yield1;
                GeneratorState::Yielded(unsafe {self.local_b.as_ref()})
            }
            State::Yield1 => {
                let local_b = unsafe { self.local_b.as_ref() };
                println!("Hello {}", local_b.iter().collect::<String>());
                self.state = State::Exit;
                GeneratorState::Complete(())
            }
            State::Exit => {
                panic!("Can't advance an exited generator!")
            }
        }
    }
}
