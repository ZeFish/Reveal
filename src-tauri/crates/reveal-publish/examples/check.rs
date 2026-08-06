//! Read-only Garden check: auth + endpoint via a dummy-hash existence GET.
//!   cargo run -p reveal-publish --example check -- ~/Documents/Atelier
fn main() {
    let vault = std::env::args().nth(1).expect("usage: check <vault>");
    let c = reveal_publish::GardenClient::from_vault(std::path::Path::new(&vault)).unwrap();
    eprintln!("api={} user={}", c.api_url, c.username);
    // un hash qui n'existe certainement pas — on attend {exists:false}
    let fake = "0".repeat(64);
    let url = format!("{}/publish/attachment?hash={fake}&ext=jpg", c.api_url);
    eprintln!("GET {url}");
    // via le client public : ensure_attachment ferait un POST si absent — on
    // fait le GET brut ici pour rester strictement lecture seule.
    let resp = ureq::get(&url)
        .set("x-api-key", &std::fs::read_to_string(
            std::path::Path::new(&vault).join(".obsidian/plugins/garden/data.json")).ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["apiKey"].as_str().map(String::from)).unwrap())
        .call();
    match resp {
        Ok(r) => eprintln!("HTTP {} → {}", r.status(), r.into_string().unwrap_or_default()),
        Err(e) => eprintln!("erreur: {e}"),
    }
}
