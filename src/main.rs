use std::path::PathBuf;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct ArgsOpt {
    #[structopt(long = "exclude-prefix")]
    exclude_prefix: Option<PathBuf>,

    #[structopt(long = "prefix")]
    prefix: Option<PathBuf>,

    #[structopt(long = "boot")]
    boot: bool,

    #[structopt(long = "create")]
    create: bool,

    #[structopt(long = "remove")]
    remove: bool,

    #[structopt(long = "clean")]
    clean: bool,

    #[structopt(long = "verbose", short = "v")]
    verbose: bool,

    #[structopt(long = "dry-run")]
    dry_run: bool,
}

#[derive(Debug, Clone)]
struct Line {
    ty: String,
    path: PathBuf,
    mode: String,
    user: String,
    group: String,
    age: String,
    argument: String,
}

fn main() {
    let args_opt = ArgsOpt::from_args();

    dbg!(args_opt);
}
