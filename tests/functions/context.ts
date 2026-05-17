/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
/// <reference path="../../functions/cloudflare-env.d.ts" />

type PagesContext = Parameters<PagesFunction<Cloudflare.Env>>[0];

const defaultEnv: Cloudflare.Env = {
    ANON_KEY: "anon-key",
    API_URL: "http://127.0.0.1:54321",
    DB_URL: "postgres://postgres:postgres@127.0.0.1:54322/postgres",
    GRAPHQL_URL: "http://127.0.0.1:54321/graphql/v1",
    INBUCKET_URL: "http://127.0.0.1:54324",
    JWT_SECRET: "test-jwt-secret",
    MAILPIT_URL: "http://127.0.0.1:54324",
    MCP_URL: "http://127.0.0.1:54321/mcp",
    PUBLISHABLE_KEY: "publishable-key",
    REST_URL: "http://127.0.0.1:54321/rest/v1",
    S3_PROTOCOL_ACCESS_KEY_ID: "access-key-id",
    S3_PROTOCOL_ACCESS_KEY_SECRET: "access-key-secret",
    S3_PROTOCOL_REGION: "local",
    SECRET_KEY: "secret-key",
    SERVICE_ROLE_KEY: "service-role-key",
    STORAGE_S3_URL: "http://127.0.0.1:54321/storage/v1/s3",
    STUDIO_URL: "http://127.0.0.1:54323",
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
