use glob::Pattern;

fn main() {
    let pattern = Pattern::new("password_*").unwrap();
    let tool_name = "password_generate";
    println!("Pattern: '{}'", "password_*");
    println!("Tool: '{}'", tool_name);
    println!("Matches: {}", pattern.matches(tool_name));
}
