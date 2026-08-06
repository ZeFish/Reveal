//! Dump the rs defaults for param-diffing against the Python reference.
fn main() {
    let p = spektrafilm_core::params::RuntimeParams::default();
    println!("{}", serde_json::to_string_pretty(&p).unwrap());
}
