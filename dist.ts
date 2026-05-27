/// <reference types="bun" />

/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import fs from "fs";
import path from "path";
import url from "url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));

const start = Date.now();

// Create dist folder if it doesn't exist
const distPath = path.join(__dirname, "dist");
if (!fs.existsSync(distPath)) {
    fs.mkdirSync(distPath);
}

// Copy index.html to dist
fs.copyFileSync(
    path.join(__dirname, "index.html"),
    path.join(distPath, "index.html"),
);

// Copy assets folder to dist
const assetsSrcPath = path.join(__dirname, "assets");
const assetsDestPath = path.join(distPath, "assets");
if (fs.existsSync(assetsDestPath)) {
    fs.rmSync(assetsDestPath, { recursive: true });
}
fs.cpSync(assetsSrcPath, assetsDestPath, { recursive: true });

// Add .nojekyll file to dist
fs.writeFileSync(path.join(distPath, ".nojekyll"), "");

// wasm-bindgen-rayon's unbundled helper imports `../../..`, which resolves to
// `/pkg/` in the browser. Wrangler serves that directory as HTML, so patch the
// generated helper to import the concrete wasm-bindgen JS module instead.
const snippetsPath = path.join(distPath, "pkg", "snippets");
let rayonWorkerHelperImportPath: string | undefined;
if (fs.existsSync(snippetsPath)) {
    for (const snippetDir of fs.readdirSync(snippetsPath)) {
        if (!snippetDir.startsWith("wasm-bindgen-rayon-")) {
            continue;
        }

        const workerHelperPath = path.join(
            snippetsPath,
            snippetDir,
            "src",
            "workerHelpers.js",
        );
        if (!fs.existsSync(workerHelperPath)) {
            continue;
        }

        const workerHelper = fs.readFileSync(workerHelperPath, "utf8");
        fs.writeFileSync(
            workerHelperPath,
            workerHelper.replace(
                "await import('../../..')",
                "await import('../../../egakareta_lib.js')",
            ),
        );
        rayonWorkerHelperImportPath = `./snippets/${snippetDir}/src/workerHelpers.js`;
    }
}

// CPAL's AudioWorklet backend imports the generated wasm-bindgen module from
// inside AudioWorkletGlobalScope. A top-level static import of the Rayon worker
// helper prevents CPAL's bundled `CpalProcessor` from registering there, so load
// the helper lazily only when `initThreadPool` calls into `startWorkers`.
const wasmBindgenJsPath = path.join(distPath, "pkg", "egakareta_lib.js");
if (rayonWorkerHelperImportPath && fs.existsSync(wasmBindgenJsPath)) {
    let wasmBindgenJs = fs.readFileSync(wasmBindgenJsPath, "utf8");
    wasmBindgenJs = wasmBindgenJs.replace(
        `import { startWorkers } from '${rayonWorkerHelperImportPath}';\n`,
        "",
    );
    wasmBindgenJs = wasmBindgenJs.replace(
        "const ret = startWorkers(arg0, arg1, wbg_rayon_PoolBuilder.__wrap(arg2));",
        `const ret = import('${rayonWorkerHelperImportPath}').then(({ startWorkers }) => startWorkers(arg0, arg1, wbg_rayon_PoolBuilder.__wrap(arg2)));`,
    );
    fs.writeFileSync(wasmBindgenJsPath, wasmBindgenJs);
}

// Add _headers file to dist
let wasmSize = 0;
const wasmPath = path.join(distPath, "pkg", "egakareta_lib_bg.wasm");
if (fs.existsSync(wasmPath)) {
    wasmSize = fs.statSync(wasmPath).size;
} else {
    console.warn(`Missing WASM artifact at ${wasmPath}`);
}

const headersContent = `
/*
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
  Cross-Origin-Resource-Policy: same-site

/pkg/egakareta_lib_bg.wasm
  x-wasm-content-length: ${wasmSize}
`;
fs.writeFileSync(path.join(distPath, "_headers"), headersContent.trim());

console.log(`created dist in ${Date.now() - start}ms`);
