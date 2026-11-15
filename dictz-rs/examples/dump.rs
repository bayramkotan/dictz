//! Print every headword and definition of a dictionary as JSON lines.
//! Used to compare the Rust parsers with the Python edition:
//! `cargo run --example dump --no-default-features -- FILE`
fn esc(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump FILE");
    let mut d = dictz::query::open_dictionary(std::path::Path::new(&path)).expect("open");
    d.load().expect("load");
    let m = d.metadata();
    println!(
        "{{\"name\":{},\"author\":{},\"description\":{},\"source\":{},\"target\":{},\"count\":{}}}",
        esc(&m.name),
        esc(&m.author),
        esc(&m.description),
        esc(&m.source_language),
        esc(&m.target_language),
        m.word_count
    );
    for e in d.entries() {
        println!("[{},{}]", esc(&e.headword), esc(&e.definition));
    }
}
