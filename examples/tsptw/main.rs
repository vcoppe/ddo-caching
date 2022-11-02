use std::{fs::File, path::Path, time::Duration};

use engineering::{
    xputils::{solve_timeout, Args, SolverType, resolution_header}, Problem, CutsetType,
};
use heuristics::{TsptwRanking, TsptwWidth};
use instance::TsptwInstance;
use model::Tsptw;
use relax::TsptwRelax;
use structopt::StructOpt;

mod heuristics;
mod instance;
mod model;
mod relax;
mod state;

fn main() {
    let args = Args::from_args();

    match args {
        Args::Solve {
            file,
            width,
            timeout,
            threads,
            solver,
            cutset,
        } => run_resolution_xp(file, width, timeout, threads, solver, cutset),
        Args::PrintHeader => resolution_header(),
    }
}

fn run_resolution_xp(
    file: String,
    width: Option<usize>,
    timeout: usize,
    threads: Option<usize>,
    solver: SolverType,
    cutset: CutsetType,
) {
    let afile = Box::new(file);
    let afile = Box::leak(afile);
    let path = Path::new(afile);
    let name = path
        .file_stem()
        .map(|s| s.to_str().unwrap_or("-- unknown --"))
        .unwrap_or("-- unknown --");
    let file = File::open(path).unwrap();
    let instance = TsptwInstance::try_from(file).unwrap();
    let model = Tsptw::new(instance);
    let relax = TsptwRelax::new(&model);
    let ranking = TsptwRanking;
    let width = TsptwWidth::new(model.nb_variables(), width.unwrap_or(1));

    let name = Box::new(name);
    let name: &'static str = Box::leak(name);
    let timeout = Duration::from_secs(timeout as u64);
    
    solve_timeout::<Tsptw, TsptwRelax, TsptwRanking, TsptwWidth>(name, timeout, &width, &model, &relax, &ranking, threads, solver, cutset);
}