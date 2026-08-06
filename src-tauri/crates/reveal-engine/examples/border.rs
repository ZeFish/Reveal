//! Preview the export paper border on a synthetic photo — no RAW needed:
//!   cargo run -p reveal-engine --example border -- out.png [border_frac]
//!
//! Lets you eyeball the matte's warmth + grain in isolation from the film
//! pipeline. Default border fraction matches the app's 4% option.
fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("usage: border <out.png> [border_frac]");
    let frac: f32 = args.next().map(|s| s.parse().unwrap()).unwrap_or(0.06);

    // A synthetic "photo": a soft diagonal gradient so the photo↔paper edge
    // and the grain in the border read clearly.
    let (w, h) = (900u32, 600u32);
    let photo = image::ImageBuffer::from_fn(w, h, |x, y| {
        let t = (x + y) as f32 / (w + h) as f32;
        let v = 30.0 + t * 150.0;
        image::Rgb([v as u8, (v * 0.86) as u8, (v * 0.72) as u8])
    });

    let bordered = reveal_engine::paper_border(&photo, frac);
    bordered.save(&out).expect("save png");
    eprintln!("wrote {out} ({}x{})", bordered.width(), bordered.height());
}
