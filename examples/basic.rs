fn main() {
    scriba_rs::run(
        scriba_rs::ScribaConfig {
            accent_color: (232, 89, 12),
            dark_mode: Some(false),
            ..Default::default()
        },
        |result| {
            let json = serde_json::to_string_pretty(&result).unwrap();
            println!("=== ScribaResult ===");
            println!("{json}");
        },
    )
    .unwrap();
}
