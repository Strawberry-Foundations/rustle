use std::fs;

pub fn open_file(path: &str) -> String {
    fs::read_to_string(path).expect("Could not read file")
}

pub fn get_language_strings(lang: &str) -> String {
    open_file(&format!("src/i18n/{lang}.yml"))
}