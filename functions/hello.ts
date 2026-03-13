export const onRequest: PagesFunction<Cloudflare.Env> = async () => {
    return new Response(JSON.stringify({ message: "Hello!" }), {
        headers: { "Content-Type": "application/json" },
    });
};
