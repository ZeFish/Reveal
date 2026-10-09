use std::path::Path;
use spektrafilm_math::image::ImageBuf;

/// Whether a parked file is whole, judged the way `parse_parked` judges it,
/// without reading the 35 MB payload: the header states the dimensions, so
/// the file's own length is the check. A park interrupted mid-write fails
/// here and gets rewritten, rather than being trusted and then refused on
/// every read for the rest of its life.
pub fn parked_file_is_complete(dest: &Path) -> bool {
    use std::io::Read;
    let Ok(meta) = std::fs::metadata(dest) else {
        return false;
    };
    let Ok(mut f) = std::fs::File::open(dest) else {
        return false;
    };
    let mut head = [0u8; 8];
    if f.read_exact(&mut head).is_err() {
        return false;
    }
    let width = u32::from_le_bytes(head[0..4].try_into().unwrap()) as u64;
    let height = u32::from_le_bytes(head[4..8].try_into().unwrap()) as u64;
    match width.checked_mul(height).and_then(|p| p.checked_mul(3 * 4)) {
        Some(pixels) => meta.len() == 8 + pixels,
        None => false,
    }
}

/// Read a parked frame back from its bytes.
///
/// The header carries the dimensions so a park interrupted mid-write, or
/// copied half-way, is REFUSED rather than fed to the pipeline as a short
/// buffer — that reads as garbage pixels, not as an error.
pub fn parse_parked(bytes: &[u8]) -> Option<ImageBuf> {
    if bytes.len() < 8 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    let pixels = &bytes[8..];
    let expected = (width as usize).checked_mul(height as usize)?.checked_mul(3)?;
    if pixels.len() != expected * 4 {
        return None;
    }
    let data: Vec<f32> = pixels
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Some(ImageBuf::from_data(width, height, data))
}

/// Parked-frame filename: the photo and the size it was parked at.
pub fn working_name(path: &Path, max_px: u32) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    // "-eb": the frame has the camera's exposure bias applied (reveal-decode
    // `raw_exposure_gain`). Frames parked by an older build do not, and must not
    // be served as if they did.
    format!("{:016x}-{max_px}-eb.buf", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn park_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut out = width.to_le_bytes().to_vec();
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&vec![0u8; (width * height * 3 * 4) as usize]);
        out
    }

    /// A whole park is recognised without reading its payload, so relaunching
    /// into the photo you were editing stops deleting and rewriting the
    /// identical 35 MB it just read.
    #[test]
    fn a_whole_park_needs_no_rewrite() {
        let dir = std::env::temp_dir().join(format!("reveal-park-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("whole.buf");
        std::fs::write(&dest, park_bytes(4, 3)).unwrap();
        assert!(parked_file_is_complete(&dest));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A park cut short must NOT be trusted: left in place it would be
    /// refused by `parse_parked` on every read for the rest of its life,
    /// costing a full decode each time and never repairing itself.
    #[test]
    fn a_park_cut_short_is_rewritten() {
        let dir = std::env::temp_dir().join(format!("reveal-park-cut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut cut = park_bytes(4, 3);
        cut.truncate(cut.len() - 40);
        let short = dir.join("short.buf");
        std::fs::write(&short, &cut).unwrap();
        assert!(!parked_file_is_complete(&short));

        let headerless = dir.join("headerless.buf");
        std::fs::write(&headerless, [1u8, 2, 3]).unwrap();
        assert!(!parked_file_is_complete(&headerless));

        assert!(!parked_file_is_complete(&dir.join("absent.buf")));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A park is keyed by the photo AND the size it was parked at. Serving one
    /// photo's buffer for another, or a 2048 buffer to a request for something
    /// else, would hand the pipeline pixels from the wrong image.
    #[test]
    fn a_parked_frame_is_keyed_by_photo_and_size() {
        let a = Path::new("/nas/2026/A.RAF");
        let b = Path::new("/nas/2026/B.RAF");
        assert_ne!(working_name(a, 2048), working_name(b, 2048));
        assert_ne!(working_name(a, 2048), working_name(a, 768));
        assert_eq!(working_name(a, 2048), working_name(a, 2048));
    }

    /// The reason the header carries dimensions: a park interrupted mid-write,
    /// or copied half-way, must be refused rather than fed to the pipeline as
    /// a short buffer — that reads as garbage pixels, not as an error.
    #[test]
    fn a_truncated_park_is_refused() {
        // A well-formed 2x2 RGB frame: 8 bytes of header, then 12 floats.
        let mut whole = Vec::new();
        whole.extend_from_slice(&2u32.to_le_bytes());
        whole.extend_from_slice(&2u32.to_le_bytes());
        for i in 0..12 {
            whole.extend_from_slice(&(i as f32).to_le_bytes());
        }
        assert_eq!(whole.len(), 56);

        let loaded = parse_parked(&whole).expect("a whole park loads");
        assert_eq!((loaded.width, loaded.height), (2, 2));
        assert_eq!(loaded.data.len(), 12);
        assert_eq!(loaded.data[11], 11.0, "pixels survive the round trip");

        assert!(parse_parked(&whole[..40]).is_none(), "cut mid-pixels");
        assert!(parse_parked(&whole[..4]).is_none(), "cut inside the header");
        assert!(parse_parked(&[]).is_none(), "empty");
        // Claims 4000x3000 but carries two pixels.
        let mut lying = Vec::new();
        lying.extend_from_slice(&4000u32.to_le_bytes());
        lying.extend_from_slice(&3000u32.to_le_bytes());
        lying.extend_from_slice(&[0u8; 24]);
        assert!(parse_parked(&lying).is_none(), "header must match the body");
    }
}
