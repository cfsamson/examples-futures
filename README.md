# Example code for the book Futures explained in 200 lines of Rust

WIP for now, the different seps we go through are put in separate branches:

- `01_naive`: A naive implementation using our own waker and task definition
- `02_proper_waker`: We change our naive waker into a proper waker
- `03_proper_future`: Instead of using a hard coded task struct, we implement the `Future` trait
- `04_future`: More changes to properly implement `Future` (double check what I thought here)
- `05_async_await`: Change our executor so we support `async/await`
- `05_final`: Make some minor adjustments so our `Futures` can be run on both `async_std` and `tokio`. **Rename branch to `06_xx`.**
- `06_bonus`: I think this is the same as `06_final` keeping it to be sure
- `07_bonus_spawn`: Take on the challanging task of implementing a `spawn` method. Talk about "blocking" in async blocks.

Each step has small changes where we go from a very simple home brewed implementaiton to implementing
our own `Futures` which can be run on both `async_std` and `tokio` runtimes.

Bonus section talks a bit more in detail about executors and runtimes, and the complexities
which stems from there.

## Important to separate between:
- `Futures` and the traits defining async in Rust.
- Runtimes and the complexity of understanding a runtime (which is not related to Rust per se, but to the runtime implementation)
- How Futures are implemented, the state machine model, `Pin` etc which we can take a look at in a later "200 lines of" explanation.

