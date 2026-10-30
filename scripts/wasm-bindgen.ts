/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repo =
    process.env.WASM_BINDGEN_REPO ??
    "https://github.com/egakareta/wasm-bindgen.git";
const ref = process.env.WASM_BINDGEN_REF ?? "main";
const cacheRoot =
    process.env.WASM_BINDGEN_CACHE ??
    path.join(os.homedir(), ".cache", "egakareta", "wasm-bindgen");
const projectRoot = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
);

export function requiredWasmBindgenVersion(lockfile: string): string {
    const versions = lockfile
        .split(/^\[\[package\]\]\s*$/m)
        .slice(1)
        .filter((packageBlock) => /^name = "wasm-bindgen"$/m.test(packageBlock))
        .map((packageBlock) => /^version = "([^"]+)"$/m.exec(packageBlock)?.[1])
        .filter((version): version is string => version !== undefined);

    if (versions.length !== 1) {
        throw new Error(
            `Expected one wasm-bindgen package in Cargo.lock, found ${versions.length}`,
        );
    }

    return versions[0];
}

export function parseWasmBindgenCliVersion(output: string): string | undefined {
    return /^wasm-bindgen\s+(\S+)/m.exec(output)?.[1];
}

function binaryVersion(binary: string): string | undefined {
    const result = Bun.spawnSync([binary, "--version"], {
        stdout: "pipe",
        stderr: "pipe",
    });
    if (result.exitCode !== 0) {
        return undefined;
    }

    return parseWasmBindgenCliVersion(
        `${result.stdout.toString()}\n${result.stderr.toString()}`,
    );
}

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
    const expectedVersion = requiredWasmBindgenVersion(
        fs.readFileSync(path.join(projectRoot, "Cargo.lock"), "utf8"),
    );
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

    const cachedVersion = fs.existsSync(binary)
        ? binaryVersion(binary)
        : undefined;
    if (cachedVersion !== expectedVersion) {
        if (cachedVersion) {
            console.log(
                `[wasm-bindgen] Cached CLI is ${cachedVersion}; project requires ${expectedVersion}. Updating it.`,
            );
        }

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

        fs.rmSync(binary, { force: true });

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

        const builtVersion = binaryVersion(binary);
        if (builtVersion !== expectedVersion) {
            throw new Error(
                `Custom wasm-bindgen ${repo}#${ref} provides ${builtVersion ?? "an unrecognized version"}, but Cargo.lock requires ${expectedVersion}. Update WASM_BINDGEN_REF or WASM_BINDGEN_REPO.`,
            );
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
