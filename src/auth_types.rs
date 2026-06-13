/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthSessionTokens {
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_at: Option<u64>,
    pub(crate) token_type: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthUser {
    pub(crate) id: String,
    pub(crate) email: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthProfile {
    pub(crate) id: String,
    pub(crate) username: Option<String>,
    pub(crate) avatar_url: Option<String>,
    pub(crate) country: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthSession {
    pub(crate) session: AuthSessionTokens,
    pub(crate) user: AuthUser,
    pub(crate) profile: Option<AuthProfile>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AuthErrorResponse {
    pub(crate) error: String,
    #[serde(default)]
    pub(crate) code: Option<String>,
}
