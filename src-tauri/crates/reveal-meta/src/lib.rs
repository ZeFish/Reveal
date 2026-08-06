//! XMP sidecars — the persistence layer for edits and library metadata.
//!
//! Convention (kept from the archive): the sidecar sits beside the original
//! as `<original>.xmp` (`IMG.RAF` → `IMG.RAF.xmp`). Standard fields use the
//! standard namespaces so Lightroom/darktable/immich read them:
//! `xmp:Rating`, `dc:description`, `dc:subject`. Reveal's own state is one
//! `reveal:EngineSettings` element holding the Recipe as JSON (schema owned
//! by reveal-engine) plus `reveal:DevelopEngine`.
//!
//! Reading is tolerant (element or attribute form, any producer — the old
//! Swift app's sidecars and Lightroom's both parse). Writing produces OUR
//! canonical document: standard fields are carried over, third-party
//! namespaces (e.g. `crs:*`) are not — the Swift app is being archived and
//! its LUT-era engine settings don't transfer anyway.

use std::io::Write;
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sidecar {
    pub rating: Option<u8>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Engine id, e.g. "spektrafilm-rs".
    pub engine: Option<String>,
    /// The engine's Recipe, opaque JSON (schema lives in reveal-engine).
    pub engine_settings: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("xml: {0}")]
    Xml(#[from] quick_xml::Error),
}

/// `IMG.RAF` → `IMG.RAF.xmp`.
pub fn sidecar_path(original: &Path) -> PathBuf {
    let mut os = original.as_os_str().to_owned();
    os.push(".xmp");
    PathBuf::from(os)
}

/// Read the sidecar next to `original`. `Ok(None)` when there is none.
pub fn read(original: &Path) -> Result<Option<Sidecar>, MetaError> {
    let path = sidecar_path(original);
    let xml = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    Ok(Some(parse(&xml)))
}

/// Write the sidecar next to `original`, atomically (tmp + rename).
pub fn write(original: &Path, sidecar: &Sidecar) -> Result<(), MetaError> {
    let path = sidecar_path(original);
    let doc = render(sidecar);
    let tmp = path.with_extension("xmp.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(doc.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// ---------------------------------------------------------------- parsing

fn parse(xml: &str) -> Sidecar {
    use quick_xml::events::Event;

    let mut out = Sidecar::default();
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Element path tracking by local name, so `dc:subject/rdf:Bag/rdf:li`
    // and friends resolve without namespace machinery.
    let mut path: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = local_name(e.name().as_ref());
                description_attributes(&mut out, &local, &e);
                path.push(local);
            }
            // Self-closing form (Lightroom writes xmp:Rating="3" on a
            // self-closed rdf:Description) — attributes only, no path push.
            Ok(Event::Empty(e)) => {
                let local = local_name(e.name().as_ref());
                description_attributes(&mut out, &local, &e);
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Text(t)) => {
                let raw = String::from_utf8_lossy(t.as_ref()).to_string();
                let text = quick_xml::escape::unescape(&raw)
                    .map(|c| c.to_string())
                    .unwrap_or(raw);
                if text.is_empty() {
                    // skip
                } else if let Some(parent) = path.last().map(String::as_str) {
                    match parent {
                        "Rating" | "DevelopEngine" | "EngineSettings" => {
                            apply_scalar(&mut out, parent, &text)
                        }
                        "li" => {
                            if path.iter().any(|p| p == "subject") {
                                out.tags.push(text);
                            } else if path.iter().any(|p| p == "description") {
                                out.description = Some(text);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// Attribute-form properties on rdf:Description (any producer).
fn description_attributes(
    out: &mut Sidecar,
    local: &str,
    e: &quick_xml::events::BytesStart<'_>,
) {
    if local != "Description" {
        return;
    }
    for attr in e.attributes().flatten() {
        let key = local_name(attr.key.as_ref());
        let val = String::from_utf8_lossy(&attr.value).to_string();
        apply_scalar(out, &key, &val);
    }
}

fn apply_scalar(out: &mut Sidecar, key: &str, val: &str) {
    match key {
        "Rating" => out.rating = val.trim().parse::<f32>().ok().map(|r| r.max(0.0) as u8),
        "DevelopEngine" => out.engine = Some(val.to_string()),
        "EngineSettings" => out.engine_settings = serde_json::from_str(val).ok(),
        _ => {}
    }
}

fn local_name(qname: &[u8]) -> String {
    let s = String::from_utf8_lossy(qname);
    s.rsplit(':').next().unwrap_or(&s).to_string()
}

// -------------------------------------------------------------- rendering

fn render(s: &Sidecar) -> String {
    let mut props = String::new();
    if let Some(r) = s.rating {
        if r > 0 {
            props.push_str(&format!("\n      <xmp:Rating>{r}</xmp:Rating>"));
        }
    }
    if let Some(d) = s.description.as_deref().filter(|d| !d.is_empty()) {
        props.push_str(&format!(
            "\n      <dc:description>\n       <rdf:Alt>\n        <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n       </rdf:Alt>\n      </dc:description>",
            escape(d)
        ));
    }
    if !s.tags.is_empty() {
        let lis: String = s
            .tags
            .iter()
            .map(|t| format!("\n        <rdf:li>{}</rdf:li>", escape(t)))
            .collect();
        props.push_str(&format!(
            "\n      <dc:subject>\n       <rdf:Bag>{lis}\n       </rdf:Bag>\n      </dc:subject>"
        ));
    }
    if let Some(engine) = s.engine.as_deref() {
        props.push_str(&format!(
            "\n      <reveal:DevelopEngine>{}</reveal:DevelopEngine>",
            escape(engine)
        ));
    }
    if let Some(settings) = &s.engine_settings {
        props.push_str(&format!(
            "\n      <reveal:EngineSettings>{}</reveal:EngineSettings>",
            escape(&settings.to_string())
        ));
    }

    format!(
        r#"<?xpacket begin="{bom}" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Reveal">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:reveal="https://reveal.photos/ns/1.0/">{props}
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#,
        bom = '\u{FEFF}',
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = Sidecar {
            rating: Some(3),
            description: Some("un café & <deux>".into()),
            tags: vec!["famille".into(), "été".into()],
            engine: Some("spektrafilm-rs".into()),
            engine_settings: Some(serde_json::json!({
                "film": "kodak_gold_200", "paper": "kodak_portra_endura",
                "grain": 1.0, "y_shift": -2.0
            })),
        };
        let parsed = parse(&render(&s));
        assert_eq!(parsed, s);
    }

    #[test]
    fn reads_swift_era_sidecar() {
        // Shape the archived Swift app produced.
        let xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Reveal">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:reveal="https://reveal.photos/ns/1.0/">
      <xmp:Rating>4</xmp:Rating>
      <dc:subject><rdf:Bag><rdf:li>portrait</rdf:li></rdf:Bag></dc:subject>
      <reveal:DevelopEngine>spektrafilm-lut</reveal:DevelopEngine>
      <reveal:EngineSettings>{"spektrafilm-lut":{"film":"kodak_gold_200"}}</reveal:EngineSettings>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;
        let s = parse(xml);
        assert_eq!(s.rating, Some(4));
        assert_eq!(s.tags, vec!["portrait".to_string()]);
        assert_eq!(s.engine.as_deref(), Some("spektrafilm-lut"));
        assert!(s.engine_settings.is_some());
    }

    #[test]
    fn reads_lightroom_attribute_form() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/"
   xmp:Rating="5"/>
 </rdf:RDF>
</x:xmpmeta>"#;
        assert_eq!(parse(xml).rating, Some(5));
    }
}
