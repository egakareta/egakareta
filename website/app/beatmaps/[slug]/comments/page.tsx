import Link from "next/link";
import { notFound } from "next/navigation";
import { beatmaps, getBeatmap, mapComments } from "@/lib/mock-data";

type BeatmapCommentsPageProps = {
    params: Promise<{ slug: string }>;
};

export function generateStaticParams() {
    return beatmaps.map((beatmap) => ({ slug: beatmap.slug }));
}

export default async function BeatmapCommentsPage({
    params,
}: BeatmapCommentsPageProps) {
    const { slug } = await params;
    const beatmap = getBeatmap(slug);

    if (!beatmap) {
        notFound();
    }

    const comments = mapComments[slug] ?? [];

    return (
        <main className="mx-auto w-[min(980px,calc(100%-1.3rem))] py-8 sm:w-[min(980px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Comments
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    {beatmap.title}
                </h1>
                <p className="mt-3 text-slate-300">
                    Community feedback, curation notes, and difficulty
                    discussion.
                </p>

                <form className="mt-6 rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                    <label className="text-sm text-slate-300">
                        Add comment
                        <textarea
                            rows={4}
                            placeholder="Share feedback about readability, sync, or difficulty..."
                            className="mt-2 w-full rounded-lg border border-cyan-100/25 bg-slate-950/70 px-3 py-2.5 text-sm text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <button
                        type="button"
                        className="mt-3 rounded-full border border-cyan-200/35 px-4 py-2 text-sm font-semibold text-cyan-200"
                    >
                        Post comment
                    </button>
                </form>

                <div className="mt-5 grid gap-3">
                    {comments.map((comment) => (
                        <article
                            key={comment.id}
                            className="rounded-xl border border-cyan-100/20 bg-slate-950/80 p-4"
                        >
                            <p className="text-sm text-slate-300">
                                <Link
                                    href={`/profiles/${comment.authorHandle}`}
                                    className="font-semibold text-cyan-300"
                                >
                                    {comment.authorHandle}
                                </Link>{" "}
                                · {comment.postedAt} · {comment.votes} votes
                            </p>
                            <p className="mt-2 leading-7 text-slate-200">
                                {comment.body}
                            </p>
                        </article>
                    ))}
                </div>

                <div className="mt-5 flex flex-wrap gap-3 text-sm">
                    <Link
                        href={`/beatmaps/${beatmap.slug}/leaderboard`}
                        className="rounded-full border border-cyan-200/35 px-4 py-2 text-cyan-200"
                    >
                        View leaderboard
                    </Link>
                    <Link
                        href={`/beatmaps/${beatmap.slug}`}
                        className="rounded-full border border-cyan-200/20 px-4 py-2 text-slate-300"
                    >
                        Back to map details
                    </Link>
                </div>
            </section>
        </main>
    );
}
