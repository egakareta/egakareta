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
