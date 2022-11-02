use std::{fs::File, path::Path, time::Duration};

use engineering::{xputils::{solve_timeout, Args, SolverType, resolution_header}, Problem, CutsetType};
use psp::PspWidth;
use structopt::StructOpt;

use crate::psp::{Psp, PspRelax, PspRanking};

mod psp;
mod utils;

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
    let model = Psp::try_from(file).unwrap();
    let relax = PspRelax;
    let ranking = PspRanking;

    let name = Box::new(name);
    let name: &'static str = Box::leak(name);
    let timeout = Duration::from_secs(timeout as u64);

    let width = PspWidth::new(model.nb_variables(), width.unwrap_or(1));
    solve_timeout::<Psp, PspRelax, PspRanking, PspWidth>(name,timeout, &width, &model, &relax, &ranking, threads,solver, cutset);
}
