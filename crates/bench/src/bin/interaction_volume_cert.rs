use clap::Parser;

fn main() {
    let args = redlinedb_bench::interaction_volume::InteractionVolumeArgs::parse();
    if let Err(error) = redlinedb_bench::interaction_volume::run(&args) {
        eprintln!("interaction-volume certification failed: {error:#}");
        std::process::exit(1);
    }
}
