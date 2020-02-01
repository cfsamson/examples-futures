# Example code for the book Futures explained in 200 lines of Rust

- `master`: The example code for the book. `Futures` can be run on both `async_std` and `tokio`.
- `basic_example_commented`: The same as the `master` branch but with extensive comments explaining everything.
- `bonus_executors`: A simple proof that using other executors runs our example as expected
- `bonus_spawn`: Implemented a very short and basic `spawn` method. 
- `vtable`: Example of a fat pointer. We create a Trait object from raw parts implementing our own vtable and data.


Each step has small changes where we go from a very simple home brewed implementaiton to implementing
our own `Futures` which can be run on both `async_std` and `tokio` runtimes.

Bonus section talks a bit more in detail about executors and runtimes, and the complexities
which stems from there.

## Important to separate between:
- `Futures` and the traits defining async in Rust.
- Runtimes and the complexity of understanding a runtime (which is not related to Rust per se, but to the runtime implementation)
- How Futures are implemented, the state machine model, `Pin` etc which we can take a look at in a later "200 lines of" explanation.

