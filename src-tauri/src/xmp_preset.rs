//! Import an Adobe XMP preset (`.xmp` out of Lightroom / Camera Raw) as a
//! Reveal preset — the same `Recipe` a Reveal preset already stores, so an
//! imported one is indistinguishable from a hand-made one afterwards.
//!
//! Two things make this more than a field rename:
//!
//! 1. **Scales differ per slider.** Lightroom is -100..100 for nearly
//!    everything; Reveal's ranges are per-control and often asymmetric
//!    (saturation is -1.0..0.5, clarity -40..60). Mapping a value to the
//!    same FRACTION of the destination's own range on that side of zero
//!    keeps "a bit of clarity" meaning a bit of clarity, which a blanket
//!    multiplier wouldn't. Exposure is the exception and is passed through
//!    untouched: both sides are in EV, and EV is EV.
//!
//! 2. **Most of what makes a preset look like itself is its tone curve.**
//!    Those arrive as 0..255 point lists, which map straight onto the
//!    engine's 0..1 control points.
//!
//! Anything Reveal has no equivalent for (noise reduction, lens profiles,
//! masks, split-toning wheels) is skipped rather than approximated — a
//! silently wrong approximation is worse than an honest omission, and the
//! report returned to the UI names what was dropped.

use reveal_engine::Recipe;

/// What an import actually managed to carry over, for the UI to show. An
/// XMP full of masks and lens corrections can import "successfully" and
/// still look nothing like it did in Lightroom; saying so is the point.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub name: String,
    /// Recipe fields this preset actually set.
    pub applied: Vec<String>,
    /// Recognized Lightroom settings Reveal has no equivalent for.
    pub skipped: Vec<String>,
}

/// Read `crs:<key>="value"`. The trailing `="` is part of the needle, so a
/// longer key sharing this one's prefix can't match by accident.
fn attr(xmp: &str, key: &str) -> Option<String> {
    let needle = format!("crs:{key}=\"");
    let start = xmp.find(&needle)? + needle.len();
    let rest = &xmp[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Read `<crs:key>value</crs:key>` — the element form some writers use for
/// the same settings others put in attributes.
fn element<'a>(xmp: &'a str, key: &str) -> Option<&'a str> {
    let open = format!("<crs:{key}>");
    let close = format!("</crs:{key}>");
    let start = xmp.find(&open)? + open.len();
    let rest = &xmp[start..];
    let end = rest.find(&close)?;
    Some(&rest[..end])
}

/// A numeric setting, from whichever of the two forms carries it.
/// Lightroom writes explicit `+` on positive values.
fn num(xmp: &str, key: &str) -> Option<f32> {
    let raw = attr(xmp, key).or_else(|| element(xmp, key).map(|s| s.to_string()))?;
    raw.trim().trim_start_matches('+').parse::<f32>().ok()
}

/// Every `<rdf:li>…</rdf:li>` inside a block.
fn seq_items(block: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(open) = rest.find("<rdf:li") {
        rest = &rest[open..];
        let Some(gt) = rest.find('>') else { break };
        rest = &rest[gt + 1..];
        let Some(close) = rest.find("</rdf:li>") else { break };
        out.push(rest[..close].trim().to_string());
        rest = &rest[close + "</rdf:li>".len()..];
    }
    out
}

/// The preset's display name, from `<crs:Name>`'s `rdf:Alt`.
fn preset_name(xmp: &str) -> Option<String> {
    let block = element(xmp, "Name")?;
    seq_items(block).into_iter().find(|s| !s.is_empty())
}

/// Lightroom's 0..255 point pairs ("x, y") as engine control points in 0..1.
/// Fewer than two points isn't a curve.
fn tone_curve(xmp: &str, key: &str) -> Option<Vec<[f32; 2]>> {
    let block = element(xmp, key)?;
    let pts: Vec<[f32; 2]> = seq_items(block)
        .iter()
        .filter_map(|s| {
            let (x, y) = s.split_once(',')?;
            Some([
                x.trim().parse::<f32>().ok()? / 255.0,
                y.trim().parse::<f32>().ok()? / 255.0,
            ])
        })
        .collect();
    (pts.len() >= 2).then_some(pts)
}

/// Map a Lightroom -100..100 value onto a destination range that may be
/// asymmetric, preserving the fraction of available travel on its own side
/// of zero.
fn scale(v: f32, neg_limit: f32, pos_limit: f32) -> f32 {
    let t = (v / 100.0).clamp(-1.0, 1.0);
    if t < 0.0 { t * neg_limit.abs() } else { t * pos_limit }
}

/// Convert an XMP document into a named recipe plus a report of what came
/// across and what didn't.
pub fn parse(xmp: &str) -> Result<(Recipe, ImportReport), String> {
    if !xmp.contains("crs:") {
        return Err("Not a Camera Raw / Lightroom preset (no crs: settings found).".into());
    }

    let mut r = Recipe::default();
    // Every field below is a Rapid-engine control; a preset that landed on
    // Spektra would silently do nothing.
    r.engine = "rapid".to_string();

    let mut applied: Vec<String> = Vec::new();
    let set = |applied: &mut Vec<String>, label: &str| applied.push(label.to_string());

    // -- tone ------------------------------------------------------------
    // EV is EV on both sides; only the engine's own limit applies.
    if let Some(v) = num(xmp, "Exposure2012") {
        r.exposure_ev = v.clamp(-3.0, 3.0);
        set(&mut applied, "Exposure");
    }
    if let Some(v) = num(xmp, "Contrast2012") {
        r.contrast = scale(v, 0.5, 0.5);
        set(&mut applied, "Contrast");
    }
    if let Some(v) = num(xmp, "Highlights2012") {
        r.highlights = scale(v, 50.0, 50.0);
        set(&mut applied, "Highlights");
    }
    if let Some(v) = num(xmp, "Shadows2012") {
        r.shadows = scale(v, 50.0, 50.0);
        set(&mut applied, "Shadows");
    }
    if let Some(v) = num(xmp, "Whites2012") {
        r.whites = scale(v, 50.0, 50.0);
        set(&mut applied, "Whites");
    }
    if let Some(v) = num(xmp, "Blacks2012") {
        r.blacks = scale(v, 50.0, 50.0);
        set(&mut applied, "Blacks");
    }

    // -- presence & color -------------------------------------------------
    if let Some(v) = num(xmp, "Clarity2012") {
        r.clarity = scale(v, 40.0, 60.0);
        set(&mut applied, "Clarity");
    }
    if let Some(v) = num(xmp, "Texture") {
        r.structure = scale(v, 30.0, 50.0);
        set(&mut applied, "Texture → Structure");
    }
    if let Some(v) = num(xmp, "Dehaze") {
        r.dehaze = scale(v, 30.0, 50.0);
        set(&mut applied, "Dehaze");
    }
    if let Some(v) = num(xmp, "Vibrance") {
        r.vibrance = scale(v, 50.0, 50.0);
        set(&mut applied, "Vibrance");
    }
    if let Some(v) = num(xmp, "Saturation") {
        r.saturation = scale(v, 1.0, 0.5);
        set(&mut applied, "Saturation");
    }

    // -- white balance ----------------------------------------------------
    // A preset carries either an absolute Kelvin (an "as shot override") or
    // an incremental -100..100 nudge. Reveal's own slider is displayed as
    // Kelvin via 5500 + v*35 (cool side) / 5500 + v*45 (warm side), so an
    // absolute temperature inverts that exact mapping rather than going
    // through mireds — this way the number Reveal shows back is the one the
    // preset asked for.
    if let Some(k) = num(xmp, "Temperature").filter(|k| *k > 1000.0) {
        let v = if k <= 5500.0 { (k - 5500.0) / 35.0 } else { (k - 5500.0) / 45.0 };
        r.temperature = v.clamp(-100.0, 100.0);
        set(&mut applied, "Temperature");
    } else if let Some(v) = num(xmp, "IncrementalTemperature") {
        r.temperature = v.clamp(-100.0, 100.0);
        set(&mut applied, "Temperature");
    }
    if let Some(v) = num(xmp, "Tint") {
        // Absolute tint runs -150..150 in Lightroom.
        r.tint = (v / 150.0 * 50.0).clamp(-50.0, 50.0);
        set(&mut applied, "Tint");
    } else if let Some(v) = num(xmp, "IncrementalTint") {
        r.tint = scale(v, 50.0, 50.0);
        set(&mut applied, "Tint");
    }

    // -- HSL ---------------------------------------------------------------
    // Same eight bands in the same order on both sides. Sat/lum share the
    // -100..100 scale; hue is -45..45 here against Lightroom's -100..100.
    const BANDS: [&str; 8] = [
        "Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta",
    ];
    let mut touched_hsl = false;
    for (i, band) in BANDS.iter().enumerate() {
        if let Some(v) = num(xmp, &format!("HueAdjustment{band}")) {
            r.hsl_hue[i] = scale(v, 45.0, 45.0);
            touched_hsl = true;
        }
        if let Some(v) = num(xmp, &format!("SaturationAdjustment{band}")) {
            r.hsl_sat[i] = v.clamp(-100.0, 100.0);
            touched_hsl = true;
        }
        if let Some(v) = num(xmp, &format!("LuminanceAdjustment{band}")) {
            r.hsl_lum[i] = v.clamp(-100.0, 100.0);
            touched_hsl = true;
        }
    }
    if touched_hsl {
        set(&mut applied, "HSL bands");
    }

    // -- tone curves --------------------------------------------------------
    // Usually the single biggest contributor to a preset's actual look.
    if let Some(pts) = tone_curve(xmp, "ToneCurvePV2012") {
        r.curve_luma = pts;
        set(&mut applied, "Tone curve");
    }
    let mut rgb_curves = false;
    if let Some(pts) = tone_curve(xmp, "ToneCurvePV2012Red") {
        r.curve_r = pts;
        rgb_curves = true;
    }
    if let Some(pts) = tone_curve(xmp, "ToneCurvePV2012Green") {
        r.curve_g = pts;
        rgb_curves = true;
    }
    if let Some(pts) = tone_curve(xmp, "ToneCurvePV2012Blue") {
        r.curve_b = pts;
        rgb_curves = true;
    }
    if rgb_curves {
        set(&mut applied, "RGB curves");
    }

    // -- grain & vignette ---------------------------------------------------
    if let Some(v) = num(xmp, "GrainAmount") {
        r.grain_amount = (v / 100.0).clamp(0.0, 1.0);
        set(&mut applied, "Grain");
    }
    if let Some(v) = num(xmp, "GrainFrequency") {
        r.grain_roughness = (0.05 + (v / 100.0).clamp(0.0, 1.0) * 0.25).clamp(0.05, 0.3);
    }
    if let Some(v) = num(xmp, "PostCropVignetteAmount") {
        r.vignette_amount = scale(v, 0.8, 0.4);
        set(&mut applied, "Vignette");
    }
    if let Some(v) = num(xmp, "PostCropVignetteMidpoint") {
        r.vignette_midpoint = (0.1 + (v / 100.0).clamp(0.0, 1.0) * 0.8).clamp(0.1, 0.9);
    }
    if let Some(v) = num(xmp, "PostCropVignetteFeather") {
        r.vignette_feather = (0.1 + (v / 100.0).clamp(0.0, 1.0) * 0.8).clamp(0.1, 0.9);
    }
    if let Some(v) = num(xmp, "PostCropVignetteRoundness") {
        r.vignette_roundness = (0.5 + (v / 100.0).clamp(-1.0, 1.0) * 0.4).clamp(0.1, 0.9);
    }

    // -- what we knowingly dropped -------------------------------------------
    let mut skipped = Vec::new();
    let note = |skipped: &mut Vec<String>, keys: &[&str], label: &str| {
        if keys.iter().any(|k| num(xmp, k).is_some_and(|v| v != 0.0)) {
            skipped.push(label.to_string());
        }
    };
    note(&mut skipped, &["LuminanceSmoothing", "ColorNoiseReduction"], "Noise reduction");
    note(&mut skipped, &["Sharpness", "SharpenRadius", "SharpenDetail"], "Sharpening");
    note(
        &mut skipped,
        &["SplitToningShadowSaturation", "SplitToningHighlightSaturation", "ColorGradeMidtoneSat"],
        "Split toning / color grading",
    );
    note(
        &mut skipped,
        &["ChromaticAberrationRedCyan", "ChromaticAberrationBlueYellow", "LensProfileEnable"],
        "Lens corrections",
    );
    if xmp.contains("crs:MaskGroupBasedCorrections") || xmp.contains("crs:CircularGradientBasedCorrections") {
        skipped.push("Masks".to_string());
    }

    let name = preset_name(xmp).unwrap_or_else(|| "Imported preset".to_string());
    if applied.is_empty() {
        return Err(format!("\u{201c}{name}\u{201d} has no settings Reveal can use."));
    }

    Ok((r, ImportReport { name, applied, skipped }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    crs:Exposure2012="+0.45"
    crs:Contrast2012="+25"
    crs:Highlights2012="-40"
    crs:Shadows2012="+30"
    crs:Saturation="-20"
    crs:Clarity2012="+10"
    crs:Tint="+15"
    crs:Temperature="6500"
    crs:HueAdjustmentRed="+20"
    crs:SaturationAdjustmentBlue="-35"
    crs:LuminanceSmoothing="25"
    crs:GrainAmount="30">
   <crs:Name>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Faded Summer</rdf:li>
    </rdf:Alt>
   </crs:Name>
   <crs:ToneCurvePV2012>
    <rdf:Seq>
     <rdf:li>0, 12</rdf:li>
     <rdf:li>128, 140</rdf:li>
     <rdf:li>255, 240</rdf:li>
    </rdf:Seq>
   </crs:ToneCurvePV2012>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn reads_the_preset_name() {
        let (_, report) = parse(SAMPLE).unwrap();
        assert_eq!(report.name, "Faded Summer");
    }

    #[test]
    fn exposure_passes_through_untouched_because_ev_is_ev() {
        let (r, _) = parse(SAMPLE).unwrap();
        assert!((r.exposure_ev - 0.45).abs() < 1e-4, "got {}", r.exposure_ev);
    }

    #[test]
    fn values_land_in_their_own_slider_range() {
        let (r, _) = parse(SAMPLE).unwrap();
        // +25 of Lightroom's 100 → a quarter of Reveal's 0.5 ceiling.
        assert!((r.contrast - 0.125).abs() < 1e-4, "contrast {}", r.contrast);
        // -40 → 40% of the 50 available below zero.
        assert!((r.highlights + 20.0).abs() < 1e-3, "highlights {}", r.highlights);
        assert!((r.shadows - 15.0).abs() < 1e-3, "shadows {}", r.shadows);
        // Saturation's range is asymmetric: -20 uses the -1.0 side.
        assert!((r.saturation + 0.2).abs() < 1e-4, "saturation {}", r.saturation);
        // Clarity's positive side runs to 60.
        assert!((r.clarity - 6.0).abs() < 1e-3, "clarity {}", r.clarity);
    }

    #[test]
    fn absolute_kelvin_inverts_reveals_own_display_mapping() {
        let (r, _) = parse(SAMPLE).unwrap();
        // 6500K is above neutral, so the warm side (5500 + v*45) applies.
        let expected = (6500.0 - 5500.0) / 45.0;
        assert!((r.temperature - expected).abs() < 1e-3, "temperature {}", r.temperature);
    }

    #[test]
    fn hsl_bands_keep_their_order_and_scale() {
        let (r, _) = parse(SAMPLE).unwrap();
        // Red is band 0; hue is -45..45 against Lightroom's -100..100.
        assert!((r.hsl_hue[0] - 9.0).abs() < 1e-3, "red hue {}", r.hsl_hue[0]);
        // Blue is band 5; saturation shares the same -100..100 scale.
        assert!((r.hsl_sat[5] + 35.0).abs() < 1e-3, "blue sat {}", r.hsl_sat[5]);
        assert_eq!(r.hsl_hue[1], 0.0, "untouched bands stay at zero");
    }

    #[test]
    fn tone_curve_points_come_across_normalized() {
        let (r, _) = parse(SAMPLE).unwrap();
        assert_eq!(r.curve_luma.len(), 3);
        assert!((r.curve_luma[0][1] - 12.0 / 255.0).abs() < 1e-4);
        assert!((r.curve_luma[2][0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn reports_what_it_could_not_carry_over() {
        let (_, report) = parse(SAMPLE).unwrap();
        assert!(report.applied.iter().any(|a| a == "Tone curve"));
        assert!(
            report.skipped.iter().any(|s| s == "Noise reduction"),
            "LuminanceSmoothing=25 should be reported as dropped, got {:?}",
            report.skipped
        );
    }

    #[test]
    fn element_form_is_read_too() {
        let xmp = r#"<rdf:Description crs:Version="15.0">
          <crs:Exposure2012>-1.25</crs:Exposure2012>
        </rdf:Description>"#;
        let (r, _) = parse(xmp).unwrap();
        assert!((r.exposure_ev + 1.25).abs() < 1e-4);
    }

    #[test]
    fn a_longer_key_sharing_a_prefix_does_not_match() {
        // `crs:Tint` must not read `crs:IncrementalTint`'s value.
        let xmp = r#"<rdf:Description crs:IncrementalTint="+90" crs:Exposure2012="0"/>"#;
        let (r, _) = parse(xmp).unwrap();
        assert!((r.tint - 45.0).abs() < 1e-3, "tint {}", r.tint);
    }

    /// The synthetic sample above is tidy. This is the shape Lightroom
    /// actually writes: settings split between attributes and elements, the
    /// full pile of defaults, `ProcessVersion`, and a curve whose points
    /// carry no spaces. Guards against a parser that only handles the
    /// format I happened to imagine.
    #[test]
    fn parses_a_real_world_lightroom_export() {
        let xmp = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 6.0-c002">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
   crs:PresetType="Normal"
   crs:Cluster=""
   crs:UUID="A1B2C3D4E5F6"
   crs:SupportsAmount="False"
   crs:SupportsColor="True"
   crs:SupportsMonochrome="True"
   crs:SupportsHighDynamicRange="True"
   crs:SupportsNormalDynamicRange="True"
   crs:SupportsSceneReferred="True"
   crs:SupportsOutputReferred="True"
   crs:CameraModelRestriction=""
   crs:Copyright=""
   crs:ContactInfo=""
   crs:Version="15.4"
   crs:ProcessVersion="15.4"
   crs:WhiteBalance="Custom"
   crs:IncrementalTemperature="+12"
   crs:IncrementalTint="-6"
   crs:Exposure2012="-0.20"
   crs:Contrast2012="+18"
   crs:Highlights2012="-62"
   crs:Shadows2012="+44"
   crs:Whites2012="-8"
   crs:Blacks2012="-14"
   crs:Texture="+12"
   crs:Clarity2012="+6"
   crs:Dehaze="+4"
   crs:Vibrance="+16"
   crs:Saturation="-8"
   crs:Sharpness="40"
   crs:LuminanceSmoothing="0"
   crs:ColorNoiseReduction="25"
   crs:HueAdjustmentOrange="-14"
   crs:SaturationAdjustmentOrange="+8"
   crs:LuminanceAdjustmentOrange="+10"
   crs:LuminanceAdjustmentAqua="-22"
   crs:PostCropVignetteAmount="-18"
   crs:PostCropVignetteMidpoint="42"
   crs:GrainAmount="18"
   crs:GrainSize="25"
   crs:GrainFrequency="50"
   crs:LensProfileEnable="1"
   crs:HasSettings="True">
   <crs:Name>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">FF — Portra Fade</rdf:li>
    </rdf:Alt>
   </crs:Name>
   <crs:ShortName>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Portra Fade</rdf:li>
    </rdf:Alt>
   </crs:ShortName>
   <crs:Group>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Francis</rdf:li>
    </rdf:Alt>
   </crs:Group>
   <crs:ToneCurvePV2012>
    <rdf:Seq>
     <rdf:li>0,18</rdf:li>
     <rdf:li>64,72</rdf:li>
     <rdf:li>190,196</rdf:li>
     <rdf:li>255,246</rdf:li>
    </rdf:Seq>
   </crs:ToneCurvePV2012>
   <crs:ToneCurvePV2012Blue>
    <rdf:Seq>
     <rdf:li>0,10</rdf:li>
     <rdf:li>255,250</rdf:li>
    </rdf:Seq>
   </crs:ToneCurvePV2012Blue>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

        let (r, report) = parse(xmp).unwrap();
        assert_eq!(report.name, "FF — Portra Fade");

        // Incremental WB, since there's no absolute Kelvin here.
        assert!((r.temperature - 12.0).abs() < 1e-3, "temperature {}", r.temperature);
        assert!((r.tint + 3.0).abs() < 1e-3, "tint {}", r.tint);

        // Orange is band 1, aqua band 4.
        assert!((r.hsl_hue[1] + 6.3).abs() < 1e-2, "orange hue {}", r.hsl_hue[1]);
        assert!((r.hsl_lum[4] + 22.0).abs() < 1e-3, "aqua lum {}", r.hsl_lum[4]);

        // The lifted-black curve is the whole look; commas with no spaces.
        assert_eq!(r.curve_luma.len(), 4);
        assert!((r.curve_luma[0][1] - 18.0 / 255.0).abs() < 1e-4);
        assert_eq!(r.curve_b.len(), 2);

        // Negative vignette uses the -0.8 side of Reveal's range.
        assert!((r.vignette_amount + 0.144).abs() < 1e-3, "vignette {}", r.vignette_amount);

        // Sharpening, colour NR and the lens profile have no home here.
        for expected in ["Sharpening", "Noise reduction", "Lens corrections"] {
            assert!(
                report.skipped.iter().any(|s| s == expected),
                "{expected} should be reported as dropped, got {:?}",
                report.skipped
            );
        }
        // …and LuminanceSmoothing="0" must not count as "had noise reduction".
        assert!(report.applied.iter().any(|a| a == "Tone curve"));
    }

    #[test]
    fn a_non_preset_file_is_refused() {
        assert!(parse("<x:xmpmeta><dc:title>hello</dc:title></x:xmpmeta>").is_err());
    }

    #[test]
    fn a_preset_with_nothing_usable_is_refused() {
        let xmp = r#"<rdf:Description crs:LuminanceSmoothing="40"/>"#;
        assert!(parse(xmp).is_err());
    }

    #[test]
    fn imported_recipes_target_the_rapid_engine() {
        let (r, _) = parse(SAMPLE).unwrap();
        assert_eq!(r.engine, "rapid");
    }
}
