/// <reference types="bun" />

/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import fs from "fs";
import path from "path";
import url from "url";

const WASM_ATOMICS_WARNING_LINES = [
    "warning: unstable feature specified for `-Ctarget-feature`: `atomics`",
    "  |",
    "  = note: this feature is not stably supported; its behavior can change in the future",
];
const WASM_FILENAME = "egakareta_lib_bg.wasm";
const WASM_PART_MANIFEST_FILENAME = `${WASM_FILENAME}.parts.json`;
const WASM_PART_SIZE = 20 * 1024 * 1024;

const rootDir = path.resolve(
    path.dirname(url.fileURLToPath(import.meta.url)),
    "..",
);

// Create dist folder if it doesn't exist
const distPath = path.join(rootDir, "dist");
if (!fs.existsSync(distPath)) {
    fs.mkdirSync(distPath);
}

fs.cpSync(path.join(rootDir, "web"), path.join(distPath), { recursive: true });

// Build the WASM package using wasm-pack
const proc = Bun.spawn(
    [
        "bunx",
        "wasm-pack",
        "build",
        "crates/eg-app",
        "-t",
        "web",
        "-d",
        "../../target/pkg",
        ...process.argv.slice(2),
    ],
    {
        stdin: "inherit",
        stdout: "inherit",
        stderr: "pipe",
        env: {
            ...process.env,
            CARGO_UNSTABLE_BUILD_STD: "std,panic_abort",
        },
    },
);

const stderrTask = forwardFilteredWasmPackStderr(proc.stderr);

// Ensure the build process is killed if this script is terminated
let shuttingDown = false;
function shutdown() {
    if (shuttingDown) return;
    shuttingDown = true;
    proc.kill();
}
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

(async () => {
    const [exitCode] = await Promise.all([proc.exited, stderrTask]);

    if (exitCode !== 0) {
        console.error(`wasm-pack exited with code ${exitCode}`);
        process.exit(exitCode || 1);
    } else {
        await createDist();
    }
})();

async function forwardFilteredWasmPackStderr(
    stderr: ReadableStream<Uint8Array> | null,
) {
    if (!stderr) {
        return;
    }

    const decoder = new TextDecoder();
    const reader = stderr.getReader();
    let bufferedText = "";
    let candidateLines: string[] = [];
    let skippingAtomicsWarningSpacing = false;

    const flushCandidateLines = () => {
        while (candidateLines.length > 0) {
            const matchesAtomicsWarningPrefix = candidateLines.every(
                (line, index) =>
                    line.replace(/\r?\n$/, "") ===
                    WASM_ATOMICS_WARNING_LINES[index],
            );

            if (matchesAtomicsWarningPrefix) {
                if (
                    candidateLines.length === WASM_ATOMICS_WARNING_LINES.length
                ) {
                    candidateLines = [];
                    skippingAtomicsWarningSpacing = true;
                }
                return;
            }

            process.stderr.write(candidateLines.shift() ?? "");
        }
    };

    const handleLine = (line: string) => {
        if (skippingAtomicsWarningSpacing) {
            if (line.trim() === "") {
                return;
            }
            skippingAtomicsWarningSpacing = false;
        }

        candidateLines.push(line);
        flushCandidateLines();
    };

    for (;;) {
        const { done, value } = await reader.read();
        if (done) {
            break;
        }

        bufferedText += decoder.decode(value, { stream: true });
        let lineEnd = bufferedText.indexOf("\n");
        while (lineEnd !== -1) {
            handleLine(bufferedText.slice(0, lineEnd + 1));
            bufferedText = bufferedText.slice(lineEnd + 1);
            lineEnd = bufferedText.indexOf("\n");
        }
    }

    bufferedText += decoder.decode();
    if (bufferedText) {
        handleLine(bufferedText);
    }
    for (const line of candidateLines) {
        process.stderr.write(line);
    }
}

async function createDist() {
    const start = Date.now();

    // Remove old pkg folder if it exists, then move the new one from target
    try {
        fs.rmSync(path.join(distPath, "pkg"), { recursive: true, force: true });
    } catch (err) {
        console.warn(`Failed to remove old pkg folder: ${err}`);
    }
    try {
        fs.renameSync(
            path.join(rootDir, "target", "pkg"),
            path.join(distPath, "pkg"),
        );
    } catch (err) {
        console.error(`Failed to move pkg folder: ${err}`);
        return;
    }

    // Copy assets folder to dist
    const assetsSrcPath = path.join(rootDir, "assets");
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

    const wasmPath = path.join(distPath, "pkg", WASM_FILENAME);
    if (!fs.existsSync(wasmPath)) {
        throw new Error(`Missing WASM artifact at ${wasmPath}`);
    }
    splitWasmForStaticHosting(wasmPath);

    // Add _headers file to dist
    const headersContent = `
/*
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
  Cross-Origin-Resource-Policy: same-site
`;
    fs.writeFileSync(path.join(distPath, "_headers"), headersContent.trim());

    console.log(`created dist in ${Date.now() - start}ms`);
}

function splitWasmForStaticHosting(wasmPath: string) {
    const wasmSize = fs.statSync(wasmPath).size;
    if (wasmSize === 0) {
        throw new Error(`Cannot split empty WASM artifact at ${wasmPath}`);
    }

    const outputDir = path.dirname(wasmPath);
    const wasmStem = path.basename(wasmPath, ".wasm");
    const wasmFile = fs.openSync(wasmPath, "r");
    const parts: { file: string; size: number }[] = [];

    try {
        for (
            let offset = 0, index = 0;
            offset < wasmSize;
            offset += WASM_PART_SIZE, index += 1
        ) {
            const partSize = Math.min(WASM_PART_SIZE, wasmSize - offset);
            const partBytes = Buffer.allocUnsafe(partSize);
            let bytesRead = 0;

            while (bytesRead < partSize) {
                const read = fs.readSync(
                    wasmFile,
                    partBytes,
                    bytesRead,
                    partSize - bytesRead,
                    offset + bytesRead,
                );
                if (read === 0) {
                    throw new Error(
                        `Unexpected end of WASM artifact at ${offset}`,
                    );
                }
                bytesRead += read;
            }

            const partFilename = `${wasmStem}.part-${index
                .toString()
                .padStart(3, "0")}.wasm`;
            fs.writeFileSync(path.join(outputDir, partFilename), partBytes);
            parts.push({ file: partFilename, size: partSize });
        }
    } finally {
        fs.closeSync(wasmFile);
    }

    fs.writeFileSync(
        path.join(outputDir, WASM_PART_MANIFEST_FILENAME),
        `${JSON.stringify({ version: 1, totalSize: wasmSize, parts }, null, 2)}\n`,
    );
    fs.unlinkSync(wasmPath);

    if (process.argv.includes("--verbose")) {
        console.log(
            `split ${WASM_FILENAME} (${wasmSize} bytes) into ${parts.length} static assets`,
        );
    }
}
