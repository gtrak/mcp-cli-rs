use std::path::Path;

fn main() {
    let path = Path::new("mcp-cli-daemon");
    println!("Path: {:?}", path);
    println!("file_name: {:?}", path.file_name());
    println!("Display: {}", path.display());
    
    let pipe_name = path.file_name().and_then(|n| n.to_str());
    println!("pipe_name: {:?}", pipe_name);
    
    let pipe_name_display = format!(r"\.\pipe\{}", pipe_name.unwrap());
    println!("pipe_name_display: {}", pipe_name_display);
}
