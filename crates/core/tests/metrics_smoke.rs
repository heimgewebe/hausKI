//! Minimaler Smoke-Test für `/metrics`.
//! 
//! Dieser Test ist bewusst als `#[ignore]` markiert, damit reguläre CI-Läufe
//! nicht scheitern, wenn kein Server läuft. Er kann gezielt aktiviert werden:
//!
//! ```bash
//! HAUSKI_TEST_BASE_URL="http://127.0.0.1:8080" \
//!   cargo test -p hauski-core --test metrics_smoke -- --ignored --nocapture
//! ```
//!
//! Erwartung: Ein laufender hausKI-Server (z. B. `hauski-cli serve`) exponiert
//! Prometheus-Metriken unter GET /metrics (Content-Type text/plain; version=0.0.4).

use reqwest::StatusCode;

fn base_url() -> String {
    std::env::var("HAUSKI_TEST_BASE_URL")
        .ok()
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
}

#[tokio::test]
#[ignore] // nur on-demand ausführen
async fn metrics_endpoint_exposes_prometheus_text() {
    let url = format!("{}/metrics", base_url().trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("request to /metrics failed");

    assert_eq!(resp.status(), StatusCode::OK, "unexpected status for /metrics");

    // Content-Type sollte Prometheus-Textformat sein
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        ctype.contains("text/plain"),
        "unexpected content-type: {}",
        ctype
    );

    let body = resp.text().await.expect("reading response body failed");
    // Heuristik: Prometheus-Textformat enthält i. d. R. HELP/TYPE-Zeilen
    assert!(
        body.contains("# HELP") || body.contains("# TYPE"),
        "unexpected /metrics payload (length={}): first bytes: {:?}",
        body.len(),
        &body.as_bytes().get(0..64)
    );
}

