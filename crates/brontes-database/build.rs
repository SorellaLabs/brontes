use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

/// sql file directory
const SQL_FILE_DIRECTORY: &str = "./queries/";

fn main() {
    write_sql();
    println!("cargo:rerun-if-changed=./queries/");
}

/// writes the sql file as a string to ./src/const_sql.rs
/// '?' are parameters that need to be bound to
fn write_sql() {
    let dest_path = Path::new("./src/database/const_sql.rs");
    let mut f = File::create(dest_path).unwrap();

    for entry in fs::read_dir(SQL_FILE_DIRECTORY).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().unwrap() == "sql" {
            let sql_string = read_sql(path.to_str().unwrap());

            let const_name = path.file_stem().unwrap().to_str().unwrap().to_uppercase();
            writeln!(f, "pub const {}: &str = r#\"{}\"#;\n", const_name, sql_string).unwrap();
        }
    }
}

// Reads an SQL file into a string
fn read_sql(s: &str) -> String {
    fs::read_to_string(s).unwrap()
}
