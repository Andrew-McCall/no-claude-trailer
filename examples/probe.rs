// Probe: does a message with no attribution always come back byte-identical?
fn main() {
    use no_claude_trailer::message::{strip, Options};
    use no_claude_trailer::profiles::Rules;
    let rules = Rules::from_names(&["claude"]).unwrap();
    for input in ["\n", "", "\n\n\n", "Subject", "Subject\n", "a\r\nb\nc\r\n"] {
        let out = strip(input, &rules, &Options::commit_object()).unwrap();
        println!(
            "{:?} -> {:?}  identical={}  hits={}",
            input,
            out.message,
            out.message == input,
            out.hits.len()
        );
    }
    // Mixed line endings with a trailer on an LF-terminated line.
    let mixed = "Subject\r\n\r\nCo-Authored-By: Claude <x@y>\nBody stays\r\n";
    let out = strip(mixed, &rules, &Options::commit_object()).unwrap();
    println!("\nmixed -> {:?} hits={}", out.message, out.hits.len());
}
