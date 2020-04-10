# Example code for the book Futures explained in 200 lines of Rust

This is the example repository for the book: [Futures Explained in 200 lines of Rust](https://github.com/cfsamson/books-futures-explained)

This example shows a self contained example of a `Future` implementation. In
addition we create a simple `Executor` and a `Reactor` which can work without
relying on real I/O which should make the example easy to play around with and
learn from.

All the code and concepts are explained thoroughly in the [accompanying book](https://github.com/cfsamson/books-futures-explained).

**There are several branches to explore:**

- `master`: The example code for the book. `Futures` can be run on both `async_std` and `tokio`.
- `basic_example_commented`: The same as the `master` branch but with extensive comments explaining everything.
- `bonus_runtimes`: A simple proof that using other executors runs our example as expected
- `vtable`: Example of a fat pointer. We create a Trait object from raw parts implementing our own vtable and data.


## Contributing

Contributions are welcome. Minor corrections, or correctness issues will be
merged to master and updated in the book, but remember that larger rewrites
needs to be updated in the book as well.

I'll create new branches for contributions showing cool, different, interesting
or improved versions of the example and point to them from the readme.

Questions and discussions are welcome in the issue tracker.