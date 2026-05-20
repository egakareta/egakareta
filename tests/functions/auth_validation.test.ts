/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { describe, expect, it } from "vitest";

import { onRequestPost as claimHandoff } from "../../functions/api/auth/handoff/claim";
import { onRequestPost as refreshSession } from "../../functions/api/auth/refresh";
import { onRequestPost as signIn } from "../../functions/api/auth/signin";
import { onRequestPost as signUp } from "../../functions/signup";
import { makePagesContext } from "./context";

function jsonPost(path: string, body: unknown) {
    return new Request(`https://egakareta.test${path}`, {
        method: "POST",
        headers: {
            Accept: "application/json",
            "Content-Type": "application/json",
        },
        body: JSON.stringify(body),
    });
}

describe("auth endpoint validation", () => {
    it("rejects sign-in without credentials", async () => {
        const response = await signIn(
            makePagesContext(jsonPost("/api/auth/signin", {})),
        );

        expect(response.status).toBe(400);
        await expect(response.json()).resolves.toEqual({
            error: "Enter your username or email and password.",
        });
    });

    it("rejects sign-in without turnstile when configured", async () => {
        const response = await signIn(
            makePagesContext(
                jsonPost("/api/auth/signin", {
                    identifier: "player@example.com",
                    password: "password123",
                }),
                { TURNSTILE_SITE_KEY: "site-key" },
            ),
        );

        expect(response.status).toBe(400);
        await expect(response.json()).resolves.toEqual({
            error: "Complete the verification challenge before signing in.",
        });
    });

    it("rejects refresh without a refresh token", async () => {
        const response = await refreshSession(
            makePagesContext(jsonPost("/api/auth/refresh", {})),
        );

        expect(response.status).toBe(400);
        await expect(response.json()).resolves.toEqual({
            error: "Missing refresh token.",
        });
    });

    it("rejects handoff claims without a valid handoff", async () => {
        const response = await claimHandoff(
            makePagesContext(jsonPost("/api/auth/handoff/claim", {})),
        );

        expect(response.status).toBe(400);
        await expect(response.json()).resolves.toEqual({
            error: "Missing or invalid sign-in handoff.",
        });
    });

    it("rejects invalid JSON sign-up data", async () => {
        const response = await signUp(
            makePagesContext(
                jsonPost("/signup", {
                    username: "no",
                    email: "not-an-email",
                    password: "short",
                }),
                { TURNSTILE_SITE_KEY: "site-key" },
            ),
        );

        expect(response.status).toBe(400);
        await expect(response.json()).resolves.toEqual({
            error: "Username must be 3-24 characters and only use letters, numbers, or underscores.",
        });
    });
});
