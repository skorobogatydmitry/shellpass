use shellpass::App;

fn main() -> eframe::Result {
    eframe::run_native("shellpass", Default::default(), Box::new(|cc| App::new(cc)))
}
