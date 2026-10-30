/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

import { expect, test } from "bun:test";

import {
    parseWasmBindgenCliVersion,
    requiredWasmBindgenVersion,
} from "./wasm-bindgen";

test("reads the wasm-bindgen version from Cargo.lock", () => {
    const lockfile = `
[[package]]
name = "wasm-bindgen-macro"
version = "0.2.130"

[[package]]
name = "wasm-bindgen"
version = "0.2.130"
source = "registry+https://github.com/rust-lang/crates.io-index"
`;

    expect(requiredWasmBindgenVersion(lockfile)).toBe("0.2.130");
});

test("requires exactly one wasm-bindgen package in Cargo.lock", () => {
    expect(() => requiredWasmBindgenVersion('name = "wasm-bindgen"')).toThrow(
        "Expected one wasm-bindgen package in Cargo.lock, found 0",
    );
    expect(() =>
        requiredWasmBindgenVersion(`
[[package]]
name = "wasm-bindgen"
version = "0.2.129"

[[package]]
name = "wasm-bindgen"
version = "0.2.130"
`),
    ).toThrow("Expected one wasm-bindgen package in Cargo.lock, found 2");
});

test("parses the version reported by the wasm-bindgen CLI", () => {
    expect(parseWasmBindgenCliVersion("wasm-bindgen 0.2.130\n")).toBe(
        "0.2.130",
    );
    expect(parseWasmBindgenCliVersion("unexpected output")).toBeUndefined();
});
