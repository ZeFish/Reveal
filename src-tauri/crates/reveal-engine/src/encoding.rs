//! The format the LUT stacks work in.
//!
//! A `.cube` is authored for one encoding of the signal: a print emulation wants Cineon, a
//! LogC3 look wants LogC3, a finishing look wants the picture as it is displayed. The recipe
//! names one format for the whole chain: the scene is encoded in it before the Pre-Lut, and the
//! render leaves in it for the Post-Lut. Nothing is converted back in between — that is the job
//! of the conversion LUTs the user puts in the stack.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LutEncoding {
    /// The picture as it is displayed: sRGB, after the AgX tone map when it leaves the render.
    #[default]
    Display,
    /// ARRI LogC3 (EI 800): a flat, dark signal made to feed a LogC-authored look.
    #[serde(rename = "logc3")]
    LogC3,
    /// Kodak Cineon: the log encoding a print-film emulation is built on.
    Cineon,
    /// Scene-linear, untouched by any curve.
    Linear,
}

impl LutEncoding {
    /// The number the GPU shader switches on (`rapid.wgsl::encode_signal`).
    pub fn code(self) -> u32 {
        match self {
            Self::Display => 0,
            Self::LogC3 => 1,
            Self::Cineon => 2,
            Self::Linear => 3,
        }
    }

    /// Encode a scene-linear value. Negative values are clamped: no log or display curve is
    /// defined below zero.
    pub fn encode(self, linear: f32) -> f32 {
        let x = linear.max(0.0);
        match self {
            Self::Display => srgb_encode(x),
            Self::LogC3 => logc3_encode(x),
            Self::Cineon => cineon_encode(x),
            Self::Linear => x,
        }
    }
}

fn srgb_encode(x: f32) -> f32 {
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

const LOGC3_CUT: f32 = 0.010591;
const LOGC3_A: f32 = 5.555556;
const LOGC3_B: f32 = 0.052272;
const LOGC3_C: f32 = 0.247190;
const LOGC3_D: f32 = 0.385537;
const LOGC3_E: f32 = 5.367655;
const LOGC3_F: f32 = 0.092809;

fn logc3_encode(x: f32) -> f32 {
    if x > LOGC3_CUT {
        LOGC3_C * (LOGC3_A * x + LOGC3_B).log10() + LOGC3_D
    } else {
        LOGC3_E * x + LOGC3_F
    }
}

/// Cineon, the usual 10-bit reading: reference white at code 685, 300 codes per decade (a
/// 0.6 negative gamma over 0.002 density per code), black at code 95. Scene 18 % grey lands at
/// about 0.457, reference white (1.0) at 0.670.
const CINEON_WHITE_CODE: f32 = 685.0;
const CINEON_CODES_PER_DECADE: f32 = 300.0;
const CINEON_BLACK_OFFSET: f32 = 0.010_792_5; // 10^((95 - 685) * 0.002 / 0.6)

fn cineon_encode(x: f32) -> f32 {
    let code = CINEON_WHITE_CODE
        + CINEON_CODES_PER_DECADE
            * (x * (1.0 - CINEON_BLACK_OFFSET) + CINEON_BLACK_OFFSET).log10();
    (code / 1023.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_grey_lands_where_each_format_puts_it() {
        assert!((LutEncoding::LogC3.encode(0.18) - 0.391).abs() < 0.002);
        assert!((LutEncoding::Cineon.encode(0.18) - 0.457).abs() < 0.003);
        assert!((LutEncoding::Display.encode(0.18) - 0.461).abs() < 0.002);
        assert_eq!(LutEncoding::Linear.encode(0.18), 0.18);
    }

    #[test]
    fn cineon_puts_black_at_code_95_and_white_at_685() {
        assert!((LutEncoding::Cineon.encode(0.0) - 95.0 / 1023.0).abs() < 0.001);
        assert!((LutEncoding::Cineon.encode(1.0) - 685.0 / 1023.0).abs() < 0.001);
    }

    #[test]
    fn a_negative_value_is_read_as_black() {
        for e in [LutEncoding::Display, LutEncoding::LogC3, LutEncoding::Cineon, LutEncoding::Linear] {
            assert_eq!(e.encode(-0.5), e.encode(0.0));
        }
    }

    #[test]
    fn the_recipe_names_are_the_lowercase_words() {
        let names: Vec<String> = [LutEncoding::Display, LutEncoding::LogC3, LutEncoding::Cineon, LutEncoding::Linear]
            .iter()
            .map(|e| serde_json::to_string(e).unwrap())
            .collect();
        assert_eq!(names, ["\"display\"", "\"logc3\"", "\"cineon\"", "\"linear\""]);
    }
}
