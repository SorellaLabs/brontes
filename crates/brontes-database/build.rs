use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

/// sql file directory
const CLICKHOUSE_FILE_DIRECTORY: &str = "./src/clickhouse/queries/";

/// sql file directory
const LIBMDBX_SQL_FILE_DIRECTORY: &str = "./src/libmdbx/tables/queries/";

fn main() {
    write_clickhouse_sql();
    write_libmdbx_sql();
}

/// writes the sql file as a string to ./src/const_sql.rs
/// '?' are parameters that need to be bound to
fn write_clickhouse_sql() {
    let dest_path = Path::new("./src/clickhouse/const_sql.rs");
    let mut f = File::create(dest_path).unwrap();

    for entry in fs::read_dir(CLICKHOUSE_FILE_DIRECTORY).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().unwrap() == "sql" {
            let sql_string = read_sql(path.to_str().unwrap());

            let const_name = path.file_stem().unwrap().to_str().unwrap().to_uppercase();
            writeln!(
                f,
                "#[allow(dead_code)]\n#[rustfmt::skip]\npub const {}: &str = r#\"{}\"#;\n",
                const_name, sql_string
            )
            .unwrap();
        }
    }
}

fn write_libmdbx_sql() {
    let dest_path = Path::new("./src/libmdbx/tables/const_sql.rs");
    let mut f = File::create(dest_path).unwrap();
    writeln!(f, "#![rustfmt::skip]\n").unwrap();

    for entry in fs::read_dir(LIBMDBX_SQL_FILE_DIRECTORY).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().unwrap() == "sql" {
            let sql_string = read_sql(path.to_str().unwrap());

            let const_name = path.file_stem().unwrap().to_str().unwrap();
            writeln!(
                f,
                "#[allow(dead_code)]\n#[allow(non_upper_case_globals)]\n#[rustfmt::skip]\npub \
                 const {}: &str = r#\"{}\"#;\n",
                const_name, sql_string
            )
            .unwrap();
        }
    }
}

// Reads an SQL file into a string
fn read_sql(s: &str) -> String {
    fs::read_to_string(s).unwrap()
}
