use hello_lib::greet;

fn main() {
    match greet("Bazel") {
        Ok(msg) => println!("{}", msg),
        Err(e) => eprintln!("error: {}", e),
    }
}
