/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

import { expect, test } from "bun:test";

import { resolveWasmGlueImports } from "./wasm-imports";

test("wasm glue binds ZenFS without importing its browser-only loader", () => {
    const source = [
        "import { configure, default as _default, fs } from '@zenfs/core';",
        'import { constants as zenfsConstants } from "@zenfs/core";',
        "import { startWorkers } from './snippets/rayon/workerHelpers.js';",
        "const backend = '@zenfs/core';",
    ].join("\n");
    const resolved = resolveWasmGlueImports(source);

    expect(resolved).toBe(
        [
            "const { configure, default: _default, fs } = globalThis.__ZENFS ?? {};",
            "const { constants: zenfsConstants } = globalThis.__ZENFS ?? {};",
            "import { startWorkers } from './snippets/rayon/workerHelpers.js';",
            "const backend = '@zenfs/core';",
        ].join("\n"),
    );
});

test("resolving wasm glue imports is idempotent", () => {
    const source = "import { fs } from '@zenfs/core';";
    const resolved = resolveWasmGlueImports(source);

    expect(resolveWasmGlueImports(resolved)).toBe(resolved);
});

test("page glue uses the configured filesystem namespace", () => {
    const source = resolveWasmGlueImports(
        "import { default as _default, fs } from '@zenfs/core';",
    );
    const load = new Function(
        "globalThis",
        `${source}\nreturn { _default, fs };`,
    );
    const zenfs = { default: {}, fs: {} };

    const bindings = load({ __ZENFS: zenfs });
    expect(bindings.fs).toBe(zenfs.fs);
    expect(bindings._default).toBe(zenfs.default);
});

test("audio/compute glue initializes without browser filesystem APIs", () => {
    const source = resolveWasmGlueImports(
        "import { default as _default, fs } from '@zenfs/core';",
    );
    const load = new Function(
        "globalThis",
        `${source}\nreturn { _default, fs };`,
    );

    expect(load({})).toEqual({ _default: undefined, fs: undefined });
});
