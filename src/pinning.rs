fn main() {
    let mut gen = GeneratorA::Enter;

    
    match gen.resume() {
        GeneratorState::Yielded(State::Yield1(n)) => {
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

struct GeneratorA<'a> {
    local_a: String,
    local_b: &'a str,
    state: State,
}

trait Generator {
    type Yield; 
    type Return;
    fn resume(&mut self) -> GeneratorState<Self::Yield, Self::Return>;
}

impl<'a> Generator for GeneratorA<'a> {
    type Yield = &'a str;
    type Return = ();
    fn resume(&mut self) -> GeneratorState<Self::Yield, Self::Return> {
        match self.state {
            State::Enter => {
                let local_a = String::from("Hello world");
                self.local_a = local_a;
                let b = &self.local_a[0..5];
                self.state = State::Yield1;
                GeneratorState::Yielded(b)
            }
            State::Yield1 => {
                let local_b = self.local_b;
                println!("Hello {}", local_b);
                self.state = State::Exit;
                GeneratorState::Complete(())
            }
            State::Exit => {
                panic!("Can't advance an exited generator!")
            }
        }
    }
}
