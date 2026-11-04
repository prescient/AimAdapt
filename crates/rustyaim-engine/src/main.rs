//! Runs the trainer directly, for development:
//! `cargo run -p rustyaim-engine -- builtin:static-grid --windowed`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(rustyaim_engine::run_from_args(&args));
}
