/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
/// Extension trait for `Result` providing a concise `.ctx()` method
/// that attaches a human-readable context prefix to errors.
pub(crate) trait MapErrContext<T, E: std::fmt::Display> {
    /// Maps the error to a `String`, prefixing it with `context`.
    ///
    /// Equivalent to `.map_err(|e| format!("{context}: {e}"))`.
    fn ctx(self, context: &str) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> MapErrContext<T, E> for Result<T, E> {
    fn ctx(self, context: &str) -> Result<T, String> {
        self.map_err(|e| format!("{context}: {e}"))
    }
}
