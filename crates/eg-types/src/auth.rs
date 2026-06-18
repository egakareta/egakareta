/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::{Deserialize, Serialize};

/// OAuth/session token data returned by the backend auth flow.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AuthSessionTokens {
    /// Bearer access token used for authenticated API requests.
    pub access_token: String,
    /// Refresh token used to renew the access token.
    pub refresh_token: String,
    /// Expiration timestamp in seconds since the Unix epoch, when known.
    pub expires_at: Option<u64>,
    /// Token type reported by the auth provider.
    pub token_type: String,
}

/// Minimal authenticated user identity.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AuthUser {
    /// Stable auth-provider user identifier.
    pub id: String,
    /// User email address, when available.
    pub email: Option<String>,
}

/// Public profile data associated with an authenticated user.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AuthProfile {
    /// Profile row identifier.
    pub id: String,
    /// Display username, when configured.
    pub username: Option<String>,
    /// Avatar URL, when configured.
    pub avatar_url: Option<String>,
    /// ISO-like country code or application default.
    pub country: String,
}

/// Complete authenticated session payload persisted by the client.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AuthSession {
    /// Session token bundle.
    pub session: AuthSessionTokens,
    /// Authenticated user identity.
    pub user: AuthUser,
    /// Optional application profile.
    pub profile: Option<AuthProfile>,
}

/// Error response shape returned by auth endpoints.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AuthErrorResponse {
    /// Human-readable error message.
    pub error: String,
    /// Optional machine-readable error code.
    #[serde(default)]
    pub code: Option<String>,
}
