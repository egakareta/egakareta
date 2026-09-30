/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

// AudioWorklets and Rayon workers import the same wasm-bindgen glue as the
// page, but have neither its import map nor its configured filesystem.
// The vendored ZenFS loader also uses top-level await and browser APIs absent
// in AudioWorklets. Bind the page's already-loaded namespace without importing
// that loader in audio/compute realms, which must not perform filesystem I/O.
export function resolveWasmGlueImports(source: string): string {
    return source.replaceAll(
        /^import\s*\{([^}]+)\}\s*from\s*(["'])@zenfs\/core\2\s*;/gm,
        (_match, bindings: string) =>
            `const {${bindings.replaceAll(/\s+as\s+/g, ": ")}} = globalThis.__ZENFS ?? {};`,
    );
}
