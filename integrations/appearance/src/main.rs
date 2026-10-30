fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [] => coconut_integration_appearance::run(),
        [flag] if flag == "--once" => coconut_integration_appearance::sync_once(),
        [flag] if flag == "--check" => coconut_theme::Store::default()
            .load()
            .and_then(|theme| coconut_integration_qt::generate(&theme).map(|_| ())),
        _ => Err("usage: coconut-appearance [--once|--check]".into()),
    };
    if let Err(error) = result {
        eprintln!("coconut-appearance: {error}");
        std::process::exit(1);
    }
}
