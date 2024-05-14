use crate::misc::art::BRONTES_BANNER;

#[allow(dead_code)]
pub fn print_banner() {
    // Read the content of the file

    println!("{}", BRONTES_BANNER);
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_print_banner() {
        print_banner();
    }
}
