/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERCIAL.md for details.

 */
import { cloudflareTest } from "@cloudflare/vitest-pool-workers";
import { defineConfig } from "vitest/config";

export default defineConfig({
    plugins: [
        cloudflareTest({
            wrangler: { configPath: "./wrangler.jsonc" },
        }),
    ],
    test: {
        coverage: {
            provider: "istanbul",
            reporter: ["text", "html", "clover", "json", "lcov"],
        },
        include: ["tests/functions/**/*.test.ts"],
    },
});
