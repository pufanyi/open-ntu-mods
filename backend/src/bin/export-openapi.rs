fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&open_ntu_mods_backend::openapi()).expect("serialize openapi")
    );
}
