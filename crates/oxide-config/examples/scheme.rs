//! Print the Material Expressive scheme for a seed: `cargo run --example scheme -- '#d4572e' [light]`
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let seed = a.first().and_then(|s| oxide_config::material::parse(s)).unwrap_or([212, 87, 46]);
    let s = oxide_config::material::scheme(seed, a.get(1).map(String::as_str) == Some("light"));
    println!("{s:?}");
}
