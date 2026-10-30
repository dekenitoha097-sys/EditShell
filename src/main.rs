#[cfg(feature = "gui")]
mod ui;

#[cfg(feature = "gui")]
fn main() {
    ui::run();
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("Compilez l’interface avec `cargo run --features gui`.");
}
