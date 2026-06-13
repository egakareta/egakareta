/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(not(target_arch = "wasm32"))]
use serde::Deserialize;
use serde::Serialize;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use web_time::Instant as PlatformInstant;

use crate::auth_types::{AuthErrorResponse, AuthSession};

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

pub(crate) fn open_signup_page() -> Result<(), String> {
    open_url(&signup_url())
}

#[cfg(target_arch = "wasm32")]
async fn sign_in_platform() -> Result<AuthSession, String> {
    open_signin_page()?;
    Err("Continue signing in on the web page.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn sign_in_platform() -> Result<AuthSession, String> {
    let start: HandoffStartResponse =
        post_json("/api/auth/handoff/start", None, &serde_json::json!({})).await?;
    open_url(&start.signin_url)?;

    let deadline = PlatformInstant::now() + Duration::from_secs(10 * 60);
    while PlatformInstant::now() < deadline {
        if let Some(session) = claim_handoff(&start.handoff_id, &start.handoff_secret).await? {
            return Ok(session);
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    Err("Sign-in timed out before the browser flow completed.".to_string())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_signin_page() -> Result<(), String> {
    open_url(&signin_url())
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(test)]
    {
        let _ = url;
        Ok(())
    }

    #[cfg(all(not(test), target_arch = "wasm32"))]
    {
        web_sys::window()
            .ok_or_else(|| "Browser window is not available.".to_string())?
            .location()
            .set_href(url)
            .map_err(|err| format!("Opening browser failed: {err:?}"))
    }

    #[cfg(all(not(test), not(target_arch = "wasm32"), target_os = "windows"))]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("Opening browser failed: {err}"))
    }

    #[cfg(all(not(test), not(target_arch = "wasm32"), target_os = "macos"))]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("Opening browser failed: {err}"))
    }

    #[cfg(all(
        not(test),
        not(target_arch = "wasm32"),
        not(target_os = "windows"),
        not(target_os = "macos")
    ))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("Opening browser failed: {err}"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn claim_handoff(
    handoff_id: &str,
    handoff_secret: &str,
) -> Result<Option<AuthSession>, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("Auth client init failed: {err}"))?;
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
            .unwrap_or_else(default_auth_base_url);

        format!("{}{}", base.trim_end_matches('/'), normalized_path.as_str())
    }
}

#[cfg(all(not(target_arch = "wasm32"), debug_assertions))]
fn default_auth_base_url() -> String {
    "http://127.0.0.1:8788".to_string()
}

#[cfg(all(not(target_arch = "wasm32"), not(debug_assertions)))]
fn default_auth_base_url() -> String {
    "https://egakareta.com".to_string()
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
    #[cfg(not(target_arch = "wasm32"))]
    use std::io::{Read, Write};
    #[cfg(not(target_arch = "wasm32"))]
    use std::net::TcpListener;
    use std::sync::{Mutex, OnceLock};
    #[cfg(not(target_arch = "wasm32"))]
    use std::thread::JoinHandle;
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::Duration as StdDuration;

    use serde::Deserialize;

    use super::{endpoint_url, parse_response, signup_url};

    #[cfg(not(target_arch = "wasm32"))]
    use super::{claim_handoff, refresh_session, sign_in, sign_out};

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

    #[cfg(not(target_arch = "wasm32"))]
    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl CapturedRequest {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(header, _)| header.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn auth_session_json(access_token: &str, refresh_token: &str) -> String {
        serde_json::json!({
            "session": {
                "access_token": access_token,
                "refresh_token": refresh_token,
                "expires_at": 123,
                "token_type": "bearer"
            },
            "user": {
                "id": "user-id",
                "email": "player@example.com"
            },
            "profile": null
        })
        .to_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_http_request(mut stream: &std::net::TcpStream) -> CapturedRequest {
        stream
            .set_read_timeout(Some(StdDuration::from_secs(2)))
            .expect("read timeout should be set");

        let mut bytes = Vec::new();
        let mut buffer = [0u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut buffer).expect("request should be read");
            assert!(read > 0, "request should include headers");
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };

        let header_text = String::from_utf8_lossy(&bytes[..header_end]);
        let mut lines = header_text.lines();
        let request_line = lines.next().expect("request line should exist");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default().to_string();
        let path = request_parts.next().unwrap_or_default().to_string();
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_string(), value.trim().to_string()))
            .collect::<Vec<_>>();
        let content_length = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.parse::<usize>().ok())
            .unwrap_or(0);

        while bytes.len() < header_end + content_length {
            let read = stream.read(&mut buffer).expect("body should be read");
            assert!(read > 0, "request body ended early");
            bytes.extend_from_slice(&buffer[..read]);
        }

        let body =
            String::from_utf8_lossy(&bytes[header_end..header_end + content_length]).to_string();

        CapturedRequest {
            method,
            path,
            headers,
            body,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn serve_once(status: u16, response_body: String) -> (String, JoinHandle<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("request should connect");
            let request = read_http_request(&stream);
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
            request
        });

        (base_url, handle)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn serve_many(responses: Vec<(u16, String)>) -> (String, JoinHandle<Vec<CapturedRequest>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::with_capacity(responses.len());
            for (status, response_body) in responses {
                let (mut stream, _) = listener.accept().expect("request should connect");
                let request = read_http_request(&stream);
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("response should write");
                requests.push(request);
            }
            requests
        });

        (base_url, handle)
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
    fn endpoint_url_uses_configured_or_default_base_when_env_is_blank() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let _env = AuthBaseUrlEnv::set("   ");

        let expected_base = option_env!("AUTH_BASE_URL")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                #[cfg(debug_assertions)]
                {
                    "http://127.0.0.1:8788"
                }

                #[cfg(not(debug_assertions))]
                {
                    "https://egakareta.com"
                }
            });

        assert_eq!(endpoint_url("signin"), format!("{expected_base}/signin"));
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn refresh_session_posts_refresh_token_and_decodes_session() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_once(200, auth_session_json("new-access", "new-refresh"));
        let _env = AuthBaseUrlEnv::set(&base_url);

        let session = pollster::block_on(refresh_session("old-refresh"))
            .expect("refresh should decode session");
        let request = handle.join().expect("server thread should finish");

        assert_eq!(session.session.access_token, "new-access");
        assert_eq!(session.session.refresh_token, "new-refresh");
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/auth/refresh");
        assert!(request.header("authorization").is_none());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.body).unwrap(),
            serde_json::json!({ "refresh_token": "old-refresh" })
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn refresh_session_returns_api_errors_from_http_response() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_once(
            401,
            serde_json::json!({ "error": "refresh token expired" }).to_string(),
        );
        let _env = AuthBaseUrlEnv::set(&base_url);

        let error = pollster::block_on(refresh_session("expired-refresh"))
            .expect_err("refresh failure should return api message");
        let request = handle.join().expect("server thread should finish");

        assert_eq!(error, "refresh token expired");
        assert_eq!(request.path, "/api/auth/refresh");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sign_out_sends_bearer_token_and_accepts_empty_response() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_once(200, "{}".to_string());
        let _env = AuthBaseUrlEnv::set(&base_url);

        pollster::block_on(sign_out(Some("access-token"))).expect("sign-out should succeed");
        let request = handle.join().expect("server thread should finish");

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/auth/signout");
        assert_eq!(request.header("authorization"), Some("Bearer access-token"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.body).unwrap(),
            serde_json::json!({})
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sign_out_omits_empty_access_token() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_once(200, "{}".to_string());
        let _env = AuthBaseUrlEnv::set(&base_url);

        pollster::block_on(sign_out(Some(""))).expect("sign-out should succeed");
        let request = handle.join().expect("server thread should finish");

        assert_eq!(request.path, "/api/auth/signout");
        assert!(request.header("authorization").is_none());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn sign_in_completes_native_handoff_start_and_claim() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_many(vec![
            (
                200,
                serde_json::json!({
                    "handoff_id": "handoff-id",
                    "handoff_secret": "handoff-secret",
                    "signin_url": "https://example.test/signin"
                })
                .to_string(),
            ),
            (200, auth_session_json("access", "refresh")),
        ]);
        let _env = AuthBaseUrlEnv::set(&base_url);

        let session = pollster::block_on(sign_in()).expect("native sign-in should claim session");
        let requests = handle.join().expect("server thread should finish");

        assert_eq!(session.session.refresh_token, "refresh");
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].path, "/api/auth/handoff/start");
        assert_eq!(requests[1].path, "/api/auth/handoff/claim");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&requests[1].body).unwrap(),
            serde_json::json!({
                "handoff_id": "handoff-id",
                "handoff_secret": "handoff-secret"
            })
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn claim_handoff_returns_none_while_pending() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_once(202, "".to_string());
        let _env = AuthBaseUrlEnv::set(&base_url);

        let claimed = pollster::block_on(claim_handoff("handoff", "secret"))
            .expect("pending handoff should not fail");
        let request = handle.join().expect("server thread should finish");

        assert_eq!(claimed, None);
        assert_eq!(request.path, "/api/auth/handoff/claim");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request.body).unwrap(),
            serde_json::json!({ "handoff_id": "handoff", "handoff_secret": "secret" })
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn claim_handoff_decodes_successful_session() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let (base_url, handle) = serve_once(200, auth_session_json("access", "refresh"));
        let _env = AuthBaseUrlEnv::set(&base_url);

        let claimed = pollster::block_on(claim_handoff("handoff", "secret"))
            .expect("handoff should decode session");
        let request = handle.join().expect("server thread should finish");

        assert_eq!(claimed.unwrap().session.access_token, "access");
        assert_eq!(request.path, "/api/auth/handoff/claim");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn claim_handoff_reports_parse_api_and_status_errors() {
        let _lock = env_lock().lock().unwrap_or_else(|error| error.into_inner());

        let (base_url, handle) = serve_once(200, "not-json".to_string());
        let _env = AuthBaseUrlEnv::set(&base_url);
        let error = pollster::block_on(claim_handoff("handoff", "secret"))
            .expect_err("invalid success response should fail");
        handle.join().expect("server thread should finish");
        assert!(error.starts_with("Auth handoff response parse failed:"));
        drop(_env);

        let (base_url, handle) = serve_once(
            403,
            serde_json::json!({ "error": "handoff expired" }).to_string(),
        );
        let _env = AuthBaseUrlEnv::set(&base_url);
        let error = pollster::block_on(claim_handoff("handoff", "secret"))
            .expect_err("api error response should fail");
        handle.join().expect("server thread should finish");
        assert_eq!(error, "handoff expired");
        drop(_env);

        let (base_url, handle) = serve_once(500, "server unavailable".to_string());
        let _env = AuthBaseUrlEnv::set(&base_url);
        let error = pollster::block_on(claim_handoff("handoff", "secret"))
            .expect_err("plain error response should fail");
        handle.join().expect("server thread should finish");
        assert_eq!(error, "Auth handoff failed with HTTP 500");
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
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| format!("Auth client init failed: {err}"))?;
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
