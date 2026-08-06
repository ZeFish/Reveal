//! Harness: extract the embedded JPEG of a RAW.
//!   cargo run -p reveal-decode --example thumb -- photo.raf out.jpg
fn main() {
    let mut a = std::env::args().skip(1);
    let (input, out) = (a.next().expect("usage: thumb <raw> <out.jpg>"), a.next().unwrap());
    let t = std::time::Instant::now();
    let jpeg = reveal_decode::extract_thumb_jpeg(std::path::Path::new(&input)).expect("extract");
    eprintln!("{} bytes en {} ms", jpeg.len(), t.elapsed().as_millis());
    std::fs::write(out, jpeg).unwrap();
}
