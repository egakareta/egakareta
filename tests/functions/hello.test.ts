/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { describe, expect, it } from "vitest";

import { onRequest } from "../../functions/hello";

describe("GET /hello", () => {
    it("returns a hello response", async () => {
        const response = await onRequest();

        expect(response.status).toBe(200);
        expect(response.headers.get("Content-Type")).toBe("application/json");
        await expect(response.json()).resolves.toEqual({ message: "Hello!" });
    });
});
