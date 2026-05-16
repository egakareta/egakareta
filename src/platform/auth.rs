/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::Serialize;

use crate::types::{AuthErrorResponse, AuthSession};

#[derive(Serialize)]
struct SignInRequest<'a> {
    identifier: &'a str,
    password: &'a str,
}

#[derive(Serialize)]
struct RefreshRequest<'a> {
    refresh_token: &'a str,
}

pub(crate) async fn sign_in(identifier: &str, password: &str) -> Result<AuthSession, String> {
    post_json(
        "/api/auth/signin",
        None,
        &SignInRequest {
            identifier,
            password,
        },
    )
    .await
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

pub(crate) fn open_signup_page() {
    let url = signup_url();

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(&url, "_blank");
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn();
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    {
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "windows"),
        not(target_os = "macos")
    ))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }
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
