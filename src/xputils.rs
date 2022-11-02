use peak_alloc::PeakAlloc;
use std::{
    fmt::Display,
    hash::Hash,
    process::exit,
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use structopt::StructOpt;

use crate::{
    InterruptibleSolver,
    ParallelSolver, Problem, Relaxation, Solver, StateRanking,
    WidthHeuristic, BarrierParallelSolver, NoDupFrontier, CutsetType,
};

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

#[derive(Debug, StructOpt)]
pub enum Args {
    Solve {
        #[structopt(short, long)]
        file: String,
        #[structopt(short, long)]
        width: Option<usize>,
        #[structopt(short, long, default_value = "60")]
        timeout: usize,
        #[structopt(short = "T", long)]
        threads: Option<usize>,
        #[structopt(short, long, default_value = "parallel")]
        solver: SolverType,
        #[structopt(short, long, default_value = "lel")]
        cutset: CutsetType,
    },
    PrintHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolverType {
    Parallel,
    Barrier,
}
impl FromStr for SolverType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "parallel" => Ok(Self::Parallel),
            "barrier" => Ok(Self::Barrier),
            _ => Err("The only supported frontier types are 'parallel' and 'barrier'"),
        }
    }
}
impl Display for SolverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parallel => write!(f, "parallel"),
            Self::Barrier => write!(f, "barrier"),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn solve_timeout<P, R, O, W>(
    name: &'static str,
    to: Duration,
    width: &W,
    model: &P,
    relax: &R,
    ranking: &O,
    threads: Option<usize>,
    solver_type: SolverType,
    cutset_type: CutsetType,
)
-> isize
where
    P: Problem + Send + Sync,
    P::State: Eq + PartialEq + Hash + Clone + Send + Sync,
    R: Relaxation<State = P::State> + Send + Sync,
    O: StateRanking<State = P::State> + Send + Sync,
    W: WidthHeuristic<P::State> + Send + Sync,
{
    let mut fringe = NoDupFrontier::new(ranking);

    match solver_type {
        SolverType::Parallel => {
            let start = Instant::now();
            let mut solver = ParallelSolver::<P, R, O, W, NoDupFrontier<O>>::custom(
                model,
                relax,
                ranking,
                width,
                cutset_type,
                &mut fringe,
                threads.unwrap_or_else(num_cpus::get),
            );
            let status = solver.maximize_with_interrupt(move || start.elapsed().gt(&to));

            let duration = start.elapsed();
            let best_value = solver
                .best_value()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "not found".to_owned());

            let lb = solver.best_lower_bound();
            let ub = solver.best_upper_bound();
            let gap = gap(lb, ub);

            println!(
                "{:>30} | {:>10} | {:>15} | {:>8.2} | {:>8.2} | {:>15} | {:>15} | {:>15} | {:>5.4} | {:>15} | {:>15}",
                name,
                solver_type,
                status,
                duration.as_secs_f32(),
                PEAK_ALLOC.peak_usage_as_mb(),
                best_value,
                lb,
                ub,
                gap,
                solver.get_explored(),
                solver.get_explored_dd(),
            );

            ub
        },
        SolverType::Barrier => {
            let start = Instant::now();
            let mut solver = BarrierParallelSolver::<P, R, O, W>::custom(
                model,
                relax,
                ranking,
                width,
                cutset_type,
                threads.unwrap_or_else(num_cpus::get),
            );
            let status = solver.maximize_with_interrupt(move || start.elapsed().gt(&to));

            let duration = start.elapsed();
            let best_value = solver
                .best_value()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "not found".to_owned());

            let lb = solver.best_lower_bound();
            let ub = solver.best_upper_bound();
            let gap = gap(lb, ub);

            println!(
                "{:>30} | {:>10} | {:>15} | {:>8.2} | {:>8.2} | {:>15} | {:>15} | {:>15} | {:>5.4} | {:>15} | {:>15}",
                name,
                solver_type,
                status,
                duration.as_secs_f32(),
                PEAK_ALLOC.peak_usage_as_mb(),
                best_value,
                lb,
                ub,
                gap,
                solver.get_explored(),
                solver.get_explored_dd(),
            );

            ub
        }
    }
}

fn gap(lb: isize, ub: isize) -> f32 {
    let aub = ub.abs();
    let alb = lb.abs();
    let u = aub.max(alb);
    let l = aub.min(alb);

    (u - l) as f32 / u as f32
}

pub fn resolution_header() {
    println!(
        "{:>30} | {:>10} | {:>15} | {:>8} | {:>8} | {:>15} | {:>15} | {:>15} | {:>5.4} | {:>15} | {:>15}",
        "NAME", "SOLVER", "STATUS", "DURATION", "RAM_(MB)", "BEST-VAL", "LB", "UB", "GAP", "NODES B&B", "NODES DD"
    );
}

pub fn timeout<A, E>(duration: Duration, mut action: A, at_exit: E)
where
    A: FnMut(),
    E: Fn() + Send + Sync + 'static,
{
    let switch = Arc::new(Mutex::new(()));
    let at_exit = Arc::new(at_exit);

    let kswitch = Arc::clone(&switch);
    let kat_exit = Arc::clone(&at_exit);

    thread::spawn(move || {
        thread::sleep(duration);

        let _lock = kswitch.lock().unwrap();
        kat_exit();
        exit(1);
    });

    action();
    let _lock = switch.lock().unwrap();
    exit(0);
}
