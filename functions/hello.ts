/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
export const onRequest: PagesFunction<Cloudflare.Env> = async () => {
    return new Response(JSON.stringify({ message: "Hello!" }), {
        headers: { "Content-Type": "application/json" },
    });
};
