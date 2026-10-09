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
//! Swift app's sidecars and Lightroom's both parse).
//!
//! Writing MERGES. The sidecar is shared ground: Lightroom, darktable and
//! others keep their own settings in it (`crs:*`, `darktable:*`, …). Reveal
//! owns exactly five properties — `xmp:Rating`, `dc:description`,
//! `dc:subject`, `reveal:DevelopEngine`, `reveal:EngineSettings` — and a write
//! replaces those and carries everything else through untouched: elements,
//! attribute values, comments, the packet wrapper. (Only the whitespace
//! between the attributes of an `rdf:Description` tag is normalised.)
//! Only when there is no sidecar yet is a fresh document written; and when an
//! existing one cannot be merged safely (not XML, no `rdf:Description`), it is
//! set aside as `<sidecar>.bak` rather than overwritten.

use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sidecar {
    pub rating: Option<u8>,
    pub capture_at: Option<i64>,
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

/// Write the sidecar next to `original`, atomically (tmp + rename), changing
/// only the properties Reveal owns (see the module docs).
pub fn write(original: &Path, sidecar: &Sidecar) -> Result<(), MetaError> {
    let path = sidecar_path(original);
    // Writing a file owns making room for it. Apple Photos edits live under a
    // per-asset directory that nothing else creates any more, now that merely
    // reading metadata no longer does.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let doc = match std::fs::read_to_string(&path) {
        Ok(existing) => match merge(&existing, sidecar) {
            Ok(merged) => merged,
            Err(_) => {
                // Not something we can edit in place. Keep what is there.
                std::fs::copy(&path, path.with_extension("xmp.bak"))?;
                render(sidecar)
            }
        },
        // Absent, or unreadable as text (then there is nothing to preserve
        // that we could read anyway).
        Err(_) => render(sidecar),
    };
    // The shared rule for writing to a disk that may be a NAS: retry, fsync,
    // read back, rename (see reveal-io).
    reveal_io::write_durable(&path, doc.as_bytes())?;
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
                        "Rating" | "DevelopEngine" | "EngineSettings" | "CaptureAt" => {
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
        "CaptureAt" => out.capture_at = val.trim().parse::<i64>().ok(),
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

/// The XML for the properties Reveal owns, ready to sit inside an `rdf:Description`.
fn props(s: &Sidecar) -> String {
    let mut props = String::new();
    if let Some(r) = s.rating {
        if r > 0 {
            props.push_str(&format!("\n      <xmp:Rating>{r}</xmp:Rating>"));
        }
    }
    if let Some(ts) = s.capture_at {
        props.push_str(&format!("\n      <reveal:CaptureAt>{ts}</reveal:CaptureAt>"));
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

    props
}

fn render(s: &Sidecar) -> String {
    let props = props(s);
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

// ------------------------------------------------------------------ merging

const NS_RDF: &[u8] = b"http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const NS_XMP: &[u8] = b"http://ns.adobe.com/xap/1.0/";
const NS_DC: &[u8] = b"http://purl.org/dc/elements/1.1/";
const NS_REVEAL: &[u8] = b"https://reveal.photos/ns/1.0/";

/// Whether (namespace, local name) is one of the properties Reveal owns.
fn is_ours(ns: &[u8], local: &[u8]) -> bool {
    (ns == NS_XMP && local == b"Rating")
        || (ns == NS_DC && (local == b"description" || local == b"subject"))
        || (ns == NS_REVEAL && (local == b"DevelopEngine" || local == b"EngineSettings" || local == b"CaptureAt"))
}

fn bound_to(ns: &quick_xml::name::ResolveResult<'_>, uri: &[u8]) -> bool {
    matches!(ns, quick_xml::name::ResolveResult::Bound(n) if n.as_ref() == uri)
}

/// `existing` with Reveal's properties replaced by `s`'s, and everything else
/// left exactly as it was. Namespaces are resolved, not guessed from prefixes,
/// so a producer that calls `dc:` something else still has its fields replaced.
///
/// Errors when the document cannot be merged safely; the caller then keeps a
/// backup and writes a fresh one.
fn merge(existing: &str, s: &Sidecar) -> Result<String, MetaError> {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
    use quick_xml::{NsReader, Writer};

    let ours = props(s);
    let mut reader = NsReader::from_str(existing);
    let mut writer = Writer::new(Vec::new());

    // Each open element: (is an rdf:Description, is the first one).
    let mut open: Vec<(bool, bool)> = Vec::new();
    let mut first_seen = false;
    // >0 while dropping one of our own child elements (and what is inside it).
    let mut skipping = 0usize;

    fn bad(msg: &str) -> MetaError {
        MetaError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_string()))
    }

    // `e` rebuilt without Reveal's attribute-form properties, and (for the one
    // Description that will receive the properties) with our namespaces declared.
    let rebuild = |reader: &NsReader<&[u8]>, e: &BytesStart<'_>, declare: bool| -> Result<BytesStart<'static>, MetaError> {
        let mut out = BytesStart::new(String::from_utf8_lossy(e.name().as_ref()).into_owned());
        let mut seen: Vec<Vec<u8>> = Vec::new();
        for attr in e.attributes() {
            let attr = attr.map_err(|e| MetaError::Xml(e.into()))?;
            let (ns, local) = reader.resolve_attribute(attr.key);
            if is_ours(
                match &ns {
                    quick_xml::name::ResolveResult::Bound(n) => n.as_ref(),
                    _ => b"",
                },
                local.as_ref(),
            ) {
                continue;
            }
            seen.push(attr.key.as_ref().to_vec());
            out.push_attribute((
                String::from_utf8_lossy(attr.key.as_ref()).into_owned().as_str(),
                attr.unescape_value().map_err(MetaError::Xml)?.into_owned().as_str(),
            ));
        }
        if declare {
            for (prefix, uri) in [("xmlns:xmp", NS_XMP), ("xmlns:dc", NS_DC), ("xmlns:reveal", NS_REVEAL)] {
                if !seen.iter().any(|k| k == prefix.as_bytes()) {
                    out.push_attribute((prefix, std::str::from_utf8(uri).unwrap_or_default()));
                }
            }
        }
        Ok(out.into_owned())
    };

    loop {
        let (ns, ev) = reader.read_resolved_event().map_err(MetaError::Xml)?;
        let is_desc = |local: &[u8]| bound_to(&ns, NS_RDF) && local == b"Description";
        let ns_bytes: Vec<u8> = match &ns {
            quick_xml::name::ResolveResult::Bound(n) => n.as_ref().to_vec(),
            _ => Vec::new(),
        };
        match ev {
            Event::Start(e) => {
                if skipping > 0 {
                    skipping += 1;
                    continue;
                }
                let local = e.local_name().as_ref().to_vec();
                if open.last().is_some_and(|(d, _)| *d) && is_ours(&ns_bytes, &local) {
                    skipping = 1;
                    continue;
                }
                if is_desc(&local) {
                    let first = !first_seen;
                    first_seen = true;
                    let rebuilt = rebuild(&reader, &e, first)?;
                    writer.write_event(Event::Start(rebuilt)).map_err(MetaError::Io)?;
                    open.push((true, first));
                } else {
                    writer.write_event(Event::Start(e.into_owned())).map_err(MetaError::Io)?;
                    open.push((false, false));
                }
            }
            Event::Empty(e) => {
                if skipping > 0 {
                    continue;
                }
                let local = e.local_name().as_ref().to_vec();
                if open.last().is_some_and(|(d, _)| *d) && is_ours(&ns_bytes, &local) {
                    continue; // a self-closed property of ours: dropped, rewritten below
                }
                if is_desc(&local) {
                    let first = !first_seen;
                    first_seen = true;
                    let rebuilt = rebuild(&reader, &e, first)?;
                    if first {
                        // `<rdf:Description .../>` has no room for children: open it up.
                        let name = rebuilt.name().as_ref().to_vec();
                        writer.write_event(Event::Start(rebuilt)).map_err(MetaError::Io)?;
                        writer.write_event(Event::Text(BytesText::from_escaped(ours.clone()))).map_err(MetaError::Io)?;
                        writer
                            .write_event(Event::End(BytesEnd::new(String::from_utf8_lossy(&name).into_owned())))
                            .map_err(MetaError::Io)?;
                    } else {
                        writer.write_event(Event::Empty(rebuilt)).map_err(MetaError::Io)?;
                    }
                } else {
                    writer.write_event(Event::Empty(e.into_owned())).map_err(MetaError::Io)?;
                }
            }
            Event::End(e) => {
                if skipping > 0 {
                    skipping -= 1;
                    continue;
                }
                if let Some((true, true)) = open.last().copied() {
                    writer.write_event(Event::Text(BytesText::from_escaped(ours.clone()))).map_err(MetaError::Io)?;
                    writer.write_event(Event::Text(BytesText::from_escaped("\n  "))).map_err(MetaError::Io)?;
                }
                open.pop();
                writer.write_event(Event::End(e.into_owned())).map_err(MetaError::Io)?;
            }
            Event::Eof => break,
            other => {
                if skipping == 0 {
                    writer.write_event(other.into_owned()).map_err(MetaError::Io)?;
                }
            }
        }
    }

    if !first_seen {
        return Err(bad("no rdf:Description to merge into"));
    }
    let merged = String::from_utf8(writer.into_inner()).map_err(|_| bad("merged document is not UTF-8"))?;
    // Trust, but verify: the file we are about to write must say what we meant,
    // and be well-formed all the way through.
    if parse(&merged) != *s {
        return Err(bad("merged document does not read back as intended"));
    }
    let mut check = quick_xml::Reader::from_str(&merged);
    loop {
        match check.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(MetaError::Xml(e)),
        }
    }
    Ok(merged)
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
            capture_at: Some(1791545900),
            description: Some("a café & <two>".into()),
            tags: vec!["family".into(), "café".into()],
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

    // ---- merging: the sidecar is shared ground -----------------------------

    const LIGHTROOM: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
    xmp:Rating="2"
    crs:Version="15.0"
    crs:Exposure2012="+0.50"
    crs:Contrast2012="+12">
   <crs:ToneCurvePV2012>
    <rdf:Seq>
     <rdf:li>0, 0</rdf:li>
     <rdf:li>255, 255</rdf:li>
    </rdf:Seq>
   </crs:ToneCurvePV2012>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

    fn ours() -> Sidecar {
        Sidecar {
            rating: Some(4),
            capture_at: None,
            description: Some("the last light".into()),
            tags: vec!["family".into()],
            engine: Some("spektra".into()),
            engine_settings: Some(serde_json::json!({ "film": "kodak_gold_200" })),
        }
    }

    #[test]
    fn merge_keeps_what_another_program_wrote() {
        let merged = merge(LIGHTROOM, &ours()).unwrap();
        // Lightroom's settings are untouched, attribute and element form alike.
        for kept in [
            r#"crs:Version="15.0""#,
            r#"crs:Exposure2012="+0.50""#,
            r#"crs:Contrast2012="+12""#,
            "<crs:ToneCurvePV2012>",
            "<rdf:li>255, 255</rdf:li>",
            r#"x:xmptk="Adobe XMP Core 7.0""#,
        ] {
            assert!(merged.contains(kept), "lost {kept}\n{merged}");
        }
        // And ours are there, once, with the rating moved out of its old attribute.
        assert_eq!(parse(&merged), ours());
        assert_eq!(merged.matches("Rating").count(), 2, "one open tag, one close tag:\n{merged}");
        assert!(!merged.contains(r#"xmp:Rating="2""#));
    }

    #[test]
    fn merge_twice_does_not_pile_up() {
        let once = merge(LIGHTROOM, &ours()).unwrap();
        let mut changed = ours();
        changed.rating = Some(1);
        changed.tags = vec!["a".into(), "b".into()];
        let twice = merge(&once, &changed).unwrap();
        assert_eq!(parse(&twice), changed);
        assert_eq!(twice.matches("<dc:subject>").count(), 1);
        assert_eq!(twice.matches("<reveal:EngineSettings>").count(), 1);
        assert!(twice.contains("crs:Exposure2012"));
    }

    #[test]
    fn merge_into_a_self_closed_description() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="5"/>
 </rdf:RDF>
</x:xmpmeta>"#;
        let merged = merge(xml, &ours()).unwrap();
        assert_eq!(parse(&merged), ours());
    }

    #[test]
    fn merge_clears_what_is_now_empty() {
        let mut none = ours();
        none.rating = None;
        none.description = None;
        none.tags.clear();
        let merged = merge(LIGHTROOM, &none).unwrap();
        assert_eq!(parse(&merged).rating, None);
        assert!(merged.contains("crs:Exposure2012"));
    }

    #[test]
    fn merge_finds_our_fields_under_other_prefixes() {
        // Same namespaces, different prefixes: still ours.
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xap="http://ns.adobe.com/xap/1.0/">
   <xap:Rating>1</xap:Rating>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;
        let merged = merge(xml, &ours()).unwrap();
        assert_eq!(parse(&merged).rating, Some(4));
        assert!(!merged.contains("<xap:Rating>"), "the old rating must go:\n{merged}");
    }

    #[test]
    fn merge_refuses_what_it_cannot_edit() {
        assert!(merge("not xml at all", &ours()).is_err());
        assert!(merge("<a><b/></a>", &ours()).is_err());
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("reveal-meta-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn write_preserves_foreign_settings_on_disk() {
        let dir = scratch("preserve");
        let raw = dir.join("IMG.RAF");
        std::fs::write(sidecar_path(&raw), LIGHTROOM).unwrap();
        write(&raw, &ours()).unwrap();
        let after = std::fs::read_to_string(sidecar_path(&raw)).unwrap();
        assert!(after.contains("crs:Exposure2012"));
        assert_eq!(read(&raw).unwrap().unwrap(), ours());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn write_sets_aside_a_sidecar_it_cannot_edit() {
        let dir = scratch("backup");
        let raw = dir.join("IMG.RAF");
        std::fs::write(sidecar_path(&raw), "this is not xml").unwrap();
        write(&raw, &ours()).unwrap();
        assert_eq!(read(&raw).unwrap().unwrap(), ours());
        let bak = std::fs::read_to_string(dir.join("IMG.RAF.xmp.bak")).unwrap();
        assert_eq!(bak, "this is not xml");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn write_without_a_sidecar_makes_a_fresh_one() {
        let dir = scratch("fresh");
        let raw = dir.join("IMG.RAF");
        write(&raw, &ours()).unwrap();
        assert_eq!(read(&raw).unwrap().unwrap(), ours());
        assert!(!dir.join("IMG.RAF.xmp.bak").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
