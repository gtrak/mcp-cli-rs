#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_compilation() {
        // These should all compile
        let patterns = ["*.git", "git-*", "*-commit", "**"];

        for pattern in patterns {
            let p = glob::Pattern::new(pattern);
            println!("Pattern '{}': compiled OK = {:?}", pattern, p.is_ok());
        }
    }
}
