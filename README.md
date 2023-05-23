# ddo-caching

This repository contains the implementation of a generic branch-and-bound algorithm based on decision diagrams.
All the code related to the classical algorithm was written by [xgillard](https://github.com/xgillard) who created the [DDO](https://github.com/xgillard/ddo) solver.

New pruning techniques detailed in an article under review were implemented in `src/mdd/with_barrier.rs` and `src/solver/barrier.rs`.

*These techniques have since been integrated to [DDO](https://github.com/xgillard/ddo), so you should better use DDO directly.*

## Requirements

You need Rust and Cargo to compile the project.
Follow the [instructions here](https://doc.rust-lang.org/cargo/getting-started/installation.html) to get them on your machine.

## Usage

For the best performance, compile the project in *release* mode with:
```
cargo build --release --all-targets
```
This will create executables in the `target/release/examples` folder for each problem specified in the `examples` folder.

If you want to learn how to solve a TSPTW (Traveling Salesman Problem with Time Windows) instance, you can then type:
```
./target/release/examples/tsptw solve --help
```
which will output:
```
USAGE:
    tsptw solve [OPTIONS] --file <file>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --cutset <cutset>       [default: lel]
    -f, --file <file>          
    -s, --solver <solver>       [default: parallel]
    -T, --threads <threads>    
    -t, --timeout <timeout>     [default: 60]
    -w, --width <width> 
```

The parameters are the following:
- `solver`: The available solvers are `parallel` and `barrier`.
They both implement the branch-and-bound algorithm based on decision diagrams but `barrier` features more pruning techniques.
- `cutset`: The `lel` and `frontier` cutsets are implemented for both algorithms.
- `width`: There is a different width strategy for each problem implemented in the `examples` folder. You can use this parameter as a multiplying factor of the width strategy.
- `timeout`: The maximum time allowed for the algorithm, in seconds.
- `threads`: The number of threads to use. *Disclaimer:* the `barrier` solver is not yet optimized for multi-threading.
- `file`: The path to the instance to solve.

The following command runs the branch-and-bound algorithm with barrier and with a frontier cutset on the instance `AFG/rbg010a.tw` on a single thread:
```
./target/release/examples/tsptw solve --solver barrier --cutset frontier --threads 1 --file resources/tsptw/AFG/rbg010a.tw
```

Three different problems are currently available in the `examples` folder:
- Traveling Salesman with Time Windows: `tsptw`
- Pigment Sequencing Problem: `psp`
- Single-Row Facility Layout Problem: `srflp`

Each with benchmark instances in the `resources` folder.

## References
+   David Bergman, Andre A. Cire, Ashish Sabharwal, Samulowitz Horst, Saraswat Vijay, and Willem-Jan and van Hoeve. [Parallel combinatorial optimization with decision diagrams.](https://link.springer.com/chapter/10.1007/978-3-319-07046-9_25) In Helmut Simonis, editor, Integration of AI and OR Techniques in Constraint Programming, volume 8451, pages 351–367. Springer, 2014.
+   David Bergman and Andre A. Cire. [Theoretical insights and algorithmic tools for decision diagram-based optimization.](https://link.springer.com/article/10.1007/s10601-016-9239-9) Constraints, 21(4):533–556, 2016.
+   David Bergman, Andre A. Cire, Willem-Jan van Hoeve, and J. N. Hooker. [Decision Diagrams for Optimization.](https://link.springer.com/book/10.1007%2F978-3-319-42849-9) Springer, 2016.
+   David Bergman, Andre A. Cire, Willem-Jan van Hoeve, and J. N. Hooker. [Discrete optimization with decision diagrams.](https://pubsonline.informs.org/doi/abs/10.1287/ijoc.2015.0648) INFORMS Journal on Computing, 28(1):47–66, 2016.
+   Xavier Gillard, Pierre Schaus, and Vianney Coppé. 2021. [Ddo, a generic and efficient framework for MDD-based optimization](https://www.ijcai.org/proceedings/2020/757). In Proceedings of the Twenty-Ninth International Joint Conference on Artificial Intelligence (IJCAI'20). Article 757, 5243–5245.
+   Xavier Gillard, Vianney Coppé, Pierre Schaus, and André Augusto Cire. 2021. [Improving the Filtering of Branch-and-Bound MDD Solver](https://link.springer.com/chapter/10.1007/978-3-030-78230-6_15). In Integration of Constraint Programming, Artificial Intelligence, and Operations Research: 18th International Conference, CPAIOR 2021, Vienna, Austria, July 5–8, 2021, Proceedings. Springer-Verlag, Berlin, Heidelberg, 231–247.
