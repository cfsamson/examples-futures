mod pinning;

fn main() {
    let mut gen = GeneratorA {
        a1: 4,
        state: State::Enter,
    };

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

// let b = |a: i32| {
//         yield a * 2;
//         println!("Hello!");
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
    Yield1(i32),
    Exit,
}

struct GeneratorA {
    a1: i32,
    state: State,
}

trait Generator {
    type Yield; 
    type Return;
    fn resume(&mut self) -> GeneratorState<Self::Yield, Self::Return>;
}

impl Generator for GeneratorA {
    type Yield = State;
    type Return = ();
    fn resume(&mut self) -> GeneratorState<Self::Yield, Self::Return> {
        match self.state {
            State::Enter => {
                self.state = State::Yield1(self.a1);
                GeneratorState::Yielded(State::Yield1(self.a1))
            }
            State::Yield1(_) => {
                println!("Hello!");
                self.state = State::Exit;
                GeneratorState::Complete(())
            }
            State::Exit => {
                panic!("Can't advance an exited generator!")
            }
        }
    }
}

// let a = |a: i32| {
//         let arr: Vec<i32> = (0..a).enumerate().map(|(i, _)| i).collect();
//         for n in arr {
//             yield n;
//         }
//         println!("The sum is: {}", arr.iter().sum());
//     };