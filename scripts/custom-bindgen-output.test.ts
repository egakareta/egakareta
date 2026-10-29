/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

import { expect, test } from "bun:test";

import { createCustomBindgenPatchOutputFilter } from "./custom-bindgen-output";

test("custom bindgen patch output is suppressed without hiding other warnings", () => {
    const shouldSuppress = createCustomBindgenPatchOutputFilter(true);

    expect(
        shouldSuppress(
            "     Locking 5 packages to latest compatible versions\n",
        ),
    ).toBe(true);
    expect(shouldSuppress("      Adding js-sys v0.3.106\n")).toBe(true);
    expect(
        shouldSuppress(
            "warning: patch `js-sys v0.3.106` was not used in the crate graph\n",
        ),
    ).toBe(true);
    expect(
        shouldSuppress(
            "help: Check that the patched package and available features are compatible\n",
        ),
    ).toBe(true);
    expect(
        shouldSuppress(
            "      with the dependency requirements. If the patch has a different version\n",
        ),
    ).toBe(true);
    expect(
        shouldSuppress(
            "warning: egakareta@0.1.0: 0 wrangler.jsonc | 2 .env.local | 3 levels\n",
        ),
    ).toBe(false);
    expect(shouldSuppress("    Finished `dev` profile\n")).toBe(false);
});

test("custom bindgen patch output filtering is disabled otherwise", () => {
    const shouldSuppress = createCustomBindgenPatchOutputFilter(false);

    expect(
        shouldSuppress(
            "     Locking 5 packages to latest compatible versions\n",
        ),
    ).toBe(false);
    expect(
        shouldSuppress(
            "warning: patch `js-sys v0.3.106` was not used in the crate graph\n",
        ),
    ).toBe(false);
});

test("custom bindgen patch filtering matches colored Cargo output", () => {
    const shouldSuppress = createCustomBindgenPatchOutputFilter(true);

    expect(
        shouldSuppress(
            "\u001b[1m\u001b[33mwarning: patch\u001b[0m `js-sys` was not used in the crate graph\n",
        ),
    ).toBe(true);
});
