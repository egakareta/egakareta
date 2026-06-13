/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
/// <reference types="bun-types" />

const commands = [
    ["dev:build", ["nodemon", "--exec", "bun run build --profiling"]],
    ["dev:wrangler", ["bunx", "wrangler", "pages", "dev"]],
] as const;

const processes = commands.map(([name, command]) => ({
    name,
    proc: Bun.spawn([...command], {
        stdin: name === "dev:wrangler" ? "inherit" : "ignore",
        stdout: "inherit",
        stderr: "inherit",
    }),
}));

let shuttingDown = false;

function shutdown() {
    if (shuttingDown) {
        return;
    }

    shuttingDown = true;

    for (const { proc } of processes) {
        proc.kill();
    }
}

process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

const firstExit = await Promise.race(
    processes.map(async ({ name, proc }) => ({
        name,
        exitCode: await proc.exited,
    })),
);

if (!shuttingDown) {
    console.error(`${firstExit.name} exited with code ${firstExit.exitCode}`);
    shutdown();
    process.exit(firstExit.exitCode || 1);
}

export {};
