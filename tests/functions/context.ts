/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
/// <reference path="../../functions/cloudflare-env.d.ts" />

type PagesContext = Parameters<PagesFunction<Cloudflare.Env>>[0];

const defaultEnv: Cloudflare.Env = {
    AUTH_BASE_URL: "https://egakareta.test",
    API_URL: "http://127.0.0.1:54321",
    PUBLISHABLE_KEY: "publishable-key",
    TURNSTILE_SITE_KEY: "",
};

export function makePagesContext(
    request: Request,
    env: Partial<Cloudflare.Env> = {},
): PagesContext {
    return {
        request,
        env: { ...defaultEnv, ...env },
        functionPath: "",
        params: {},
        data: {},
        waitUntil: () => undefined,
        passThroughOnException: () => undefined,
        next: () => Promise.resolve(new Response(null, { status: 404 })),
    } as unknown as PagesContext;
}
