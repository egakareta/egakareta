/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

use crate::types::{AuthErrorResponse, AuthSession};

#[derive(Serialize)]
struct RefreshRequest<'a> {
    refresh_token: &'a str,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Deserialize)]
struct HandoffStartResponse {
    handoff_id: String,
    handoff_secret: String,
    signin_url: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Serialize)]
struct HandoffClaimRequest<'a> {
    handoff_id: &'a str,
    handoff_secret: &'a str,
}

pub(crate) async fn sign_in() -> Result<AuthSession, String> {
    sign_in_platform().await
}

pub(crate) async fn refresh_session(refresh_token: &str) -> Result<AuthSession, String> {
    post_json("/api/auth/refresh", None, &RefreshRequest { refresh_token }).await
}

pub(crate) async fn sign_out(access_token: Option<&str>) -> Result<(), String> {
    let _: serde_json::Value =
        post_json("/api/auth/signout", access_token, &serde_json::json!({})).await?;
    Ok(())
}

pub(crate) fn signup_url() -> String {
    endpoint_url("/signup")
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn signin_url() -> String {
    endpoint_url("/signin")
}

pub(crate) fn open_signup_page() {
    open_url(&signup_url());
}

#[cfg(target_arch = "wasm32")]
async fn sign_in_platform() -> Result<AuthSession, String> {
    open_signin_page();
    Err("Continue signing in on the web page.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn sign_in_platform() -> Result<AuthSession, String> {
    let start: HandoffStartResponse =
        post_json("/api/auth/handoff/start", None, &serde_json::json!({})).await?;
    open_url(&start.signin_url);

    let deadline = Instant::now() + Duration::from_secs(10 * 60);
    while Instant::now() < deadline {
        if let Some(session) = claim_handoff(&start.handoff_id, &start.handoff_secret).await? {
            return Ok(session);
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    Err("Sign-in timed out before the browser flow completed.".to_string())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_signin_page() {
    open_url(&signin_url());
}

fn open_url(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.location().set_href(url);
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "windows"),
        not(target_os = "macos")
    ))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn claim_handoff(
    handoff_id: &str,
    handoff_secret: &str,
) -> Result<Option<AuthSession>, String> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(endpoint_url("/api/auth/handoff/claim"))
        .json(&HandoffClaimRequest {
            handoff_id,
            handoff_secret,
        })
        .send()
        .map_err(|err| format!("Auth handoff claim failed: {err}"))?;
    let status = response.status().as_u16();
    let text = response
        .text()
        .map_err(|err| format!("Auth handoff response read failed: {err}"))?;

    if status == 202 {
        return Ok(None);
    }
    if (200..300).contains(&status) {
        return serde_json::from_str(&text)
            .map(Some)
            .map_err(|err| format!("Auth handoff response parse failed: {err}"));
    }

    if let Ok(error) = serde_json::from_str::<AuthErrorResponse>(&text) {
        return Err(error.error);
    }

    Err(format!("Auth handoff failed with HTTP {status}"))
}

async fn post_json<T, R>(path: &str, access_token: Option<&str>, body: &T) -> Result<R, String>
where
    T: Serialize + ?Sized,
    R: serde::de::DeserializeOwned,
{
    post_json_platform(path, access_token, body).await
}

fn endpoint_url(path: &str) -> String {
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };

    #[cfg(target_arch = "wasm32")]
    {
        normalized_path
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let base = std::env::var("EGAKARETA_AUTH_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                option_env!("AUTH_BASE_URL")
                    .map(str::to_string)
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(|| "http://127.0.0.1:8788".to_string());

        format!("{}{}", base.trim_end_matches('/'), normalized_path.as_str())
    }
}

fn parse_response<R>(status: u16, text: &str) -> Result<R, String>
where
    R: serde::de::DeserializeOwned,
{
    if (200..300).contains(&status) {
        return serde_json::from_str(text)
            .map_err(|err| format!("Auth response parse failed: {err}"));
    }

    if let Ok(error) = serde_json::from_str::<AuthErrorResponse>(text) {
        return Err(error.error);
    }

    Err(format!("Auth request failed with HTTP {status}"))
}

#[cfg(target_arch = "wasm32")]
async fn post_json_platform<T, R>(
    path: &str,
    access_token: Option<&str>,
    body: &T,
) -> Result<R, String>
where
    T: Serialize + ?Sized,
    R: serde::de::DeserializeOwned,
{
    use gloo_net::http::Request;

    let body =
        serde_json::to_string(body).map_err(|err| format!("Auth JSON encode failed: {err}"))?;
    let mut request = Request::post(&endpoint_url(path)).header("Content-Type", "application/json");
    if let Some(token) = access_token.filter(|token| !token.is_empty()) {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }

    let response = request
        .body(body)
        .map_err(|err| format!("Auth request build failed: {err}"))?
        .send()
        .await
        .map_err(|err| format!("Auth request failed: {err}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|err| format!("Auth response read failed: {err}"))?;

    parse_response(status, &text)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use serde::Deserialize;

    use super::{endpoint_url, parse_response, signup_url};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct AuthBaseUrlEnv {
        previous: Option<String>,
    }

    impl AuthBaseUrlEnv {
        fn set(value: &str) -> Self {
            let previous = std::env::var("EGAKARETA_AUTH_BASE_URL").ok();
            std::env::set_var("EGAKARETA_AUTH_BASE_URL", value);
            Self { previous }
        }
    }

    impl Drop for AuthBaseUrlEnv {
        fn drop(&mut self) {
            if let Some(previous) = &self.previous {
                std::env::set_var("EGAKARETA_AUTH_BASE_URL", previous);
            } else {
                std::env::remove_var("EGAKARETA_AUTH_BASE_URL");
            }
        }
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct ParsedPayload {
        value: u32,
    }

    #[test]
    fn endpoint_url_normalizes_paths_and_trims_custom_base() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let _env = AuthBaseUrlEnv::set("https://auth.example.test/root/");

        assert_eq!(
            endpoint_url("api/auth/refresh"),
            "https://auth.example.test/root/api/auth/refresh"
        );
        assert_eq!(signup_url(), "https://auth.example.test/root/signup");
    }

    #[test]
    fn parse_response_decodes_successful_json() {
        let parsed: ParsedPayload =
            parse_response(200, r#"{"value":7}"#).expect("successful auth response should parse");

        assert_eq!(parsed, ParsedPayload { value: 7 });
    }

    #[test]
    fn parse_response_reports_success_json_parse_errors() {
        let error = parse_response::<ParsedPayload>(200, "not-json")
            .expect_err("invalid success body should be rejected");

        assert!(error.starts_with("Auth response parse failed:"));
    }

    #[test]
    fn parse_response_prefers_api_error_message() {
        let error =
            parse_response::<ParsedPayload>(401, r#"{"error":"session expired","code":"401"}"#)
                .expect_err("auth error payload should be returned");

        assert_eq!(error, "session expired");
    }

    #[test]
    fn parse_response_falls_back_to_http_status() {
        let error = parse_response::<ParsedPayload>(503, "temporarily unavailable")
            .expect_err("plain error body should include status");

        assert_eq!(error, "Auth request failed with HTTP 503");
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn post_json_platform<T, R>(
    path: &str,
    access_token: Option<&str>,
    body: &T,
) -> Result<R, String>
where
    T: Serialize + ?Sized,
    R: serde::de::DeserializeOwned,
{
    let client = reqwest::blocking::Client::new();
    let mut request = client.post(endpoint_url(path)).json(body);
    if let Some(token) = access_token.filter(|token| !token.is_empty()) {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .map_err(|err| format!("Auth request failed: {err}"))?;
    let status = response.status().as_u16();
    let text = response
        .text()
        .map_err(|err| format!("Auth response read failed: {err}"))?;

    parse_response(status, &text)
}
