/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const repo =
    process.env.WASM_BINDGEN_REPO ??
    "https://github.com/egakareta/wasm-bindgen.git";
const ref = process.env.WASM_BINDGEN_REF ?? "main";
const cacheRoot =
    process.env.WASM_BINDGEN_CACHE ??
    path.join(os.homedir(), ".cache", "egakareta", "wasm-bindgen");

function run(
    command: string[],
    options: { cwd?: string; env?: Record<string, string | undefined> } = {},
): Promise<number> {
    const child = Bun.spawn(command, {
        cwd: options.cwd,
        env: options.env,
        stdin: "inherit",
        stdout: "inherit",
        stderr: "inherit",
    });
    return child.exited;
}

function keyFor(value: string): string {
    return createHash("sha256").update(value).digest("hex").slice(0, 20);
}

export async function getBinary() {
    const rustc = Bun.spawnSync(["rustc", "-vV"], {
        stdout: "pipe",
        stderr: "inherit",
    });
    if (rustc.exitCode !== 0) {
        process.exit(rustc.exitCode ?? 1);
    }

    const cacheKey = keyFor(
        [repo, ref, os.platform(), os.arch(), rustc.stdout.toString()].join(
            "\n",
        ),
    );
    const checkout = path.join(cacheRoot, `src-${cacheKey}`);
    const binary = path.join(
        checkout,
        "target",
        "release",
        process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen",
    );

    fs.mkdirSync(cacheRoot, { recursive: true });

    if (!fs.existsSync(binary)) {
        if (!fs.existsSync(path.join(checkout, ".git"))) {
            fs.rmSync(checkout, { recursive: true, force: true });

            const cloneExit = await run([
                "git",
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                repo,
                checkout,
            ]);
            if (cloneExit !== 0) process.exit(cloneExit);
        }

        let exit = await run(["git", "fetch", "--depth", "1", "origin", ref], {
            cwd: checkout,
        });
        if (exit !== 0) process.exit(exit);

        exit = await run(["git", "checkout", "--detach", "FETCH_HEAD"], {
            cwd: checkout,
        });
        if (exit !== 0) process.exit(exit);

        const rustflags = [process.env.RUSTFLAGS, "-C target-cpu=native"]
            .filter(Boolean)
            .join(" ");

        exit = await run(
            [
                "cargo",
                "build",
                "--release",
                "--package",
                "wasm-bindgen-cli",
                "--bin",
                "wasm-bindgen",
            ],
            {
                cwd: checkout,
                env: {
                    ...process.env,
                    RUSTFLAGS: rustflags,
                    CARGO_PROFILE_RELEASE_LTO: "fat",
                    CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
                    CARGO_PROFILE_RELEASE_STRIP: "symbols",
                },
            },
        );
        if (exit !== 0) process.exit(exit);

        if (!fs.existsSync(binary)) {
            throw new Error(`Build succeeded but binary is missing: ${binary}`);
        }
    }
    return { binary, checkout };
}

async function main() {
    const binary = (await getBinary()).binary;
    const child = Bun.spawn([binary, ...process.argv.slice(2)], {
        stdin: "inherit",
        stdout: "inherit",
        stderr: "inherit",
    });
    process.exit(await child.exited);
}

if (import.meta.main) {
    main().catch((error) => {
        console.error(error);
        process.exit(1);
    });
}
