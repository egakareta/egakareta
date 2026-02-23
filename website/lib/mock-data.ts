export type BeatmapStatus = "Unranked" | "Ranked" | "Official";

export type Beatmap = {
    slug: string;
    title: string;
    artist: string;
    mapper: string;
    mapperHandle: string;
    length: string;
    bpm: number;
    nominatorStars: number;
    communityStars: number;
    status: BeatmapStatus;
    downloads: number;
    rating: number;
    tags: string[];
    description: string;
    uploadedAt: string;
};

export type ScoreEntry = {
    rank: number;
    playerHandle: string;
    accuracy: string;
    score: string;
    pp: number;
    mods: string;
    playedAt: string;
};

export type CommentEntry = {
    id: string;
    authorHandle: string;
    postedAt: string;
    body: string;
    votes: number;
};

export type PpEntry = {
    rank: number;
    handle: string;
    country: string;
    pp: number;
    accuracy: string;
    rankedScore: string;
    mapsCleared: number;
};

export type Profile = {
    handle: string;
    displayName: string;
    bio: string;
    country: string;
    joinedAt: string;
    followerCount: number;
    mapperTier: "New Mapper" | "Nominator" | "Ranked Mapper";
    totalPp: number;
    globalRank: number;
    favoriteMods: string[];
};

export const beatmaps: Beatmap[] = [
    {
        slug: "railbreaker",
        title: "Railbreaker",
        artist: "Haru Komaki",
        mapper: "Nova",
        mapperHandle: "nova",
        length: "2:28",
        bpm: 174,
        nominatorStars: 5.8,
        communityStars: 5.4,
        status: "Ranked",
        downloads: 10234,
        rating: 4.6,
        tags: ["speed", "precision", "sync-heavy"],
        description:
            "Fast lane changes with narrow wall patterns built around vocal chops and kick accents.",
        uploadedAt: "2026-02-02",
    },
    {
        slug: "neon-archive",
        title: "Neon Archive",
        artist: "Yume Circuit",
        mapper: "Iris",
        mapperHandle: "iris",
        length: "3:05",
        bpm: 160,
        nominatorStars: 4.3,
        communityStars: 4.7,
        status: "Unranked",
        downloads: 5190,
        rating: 4.4,
        tags: ["flow", "editor-showcase", "mid-difficulty"],
        description:
            "A scenic route map featuring ramps, moving hazards, and layered rhythm cues.",
        uploadedAt: "2026-02-14",
    },
    {
        slug: "first-light",
        title: "First Light",
        artist: "Line Dash Team",
        mapper: "Official",
        mapperHandle: "official",
        length: "1:52",
        bpm: 142,
        nominatorStars: 2.1,
        communityStars: 2.0,
        status: "Official",
        downloads: 23540,
        rating: 4.8,
        tags: ["tutorial", "intro", "official"],
        description:
            "Core mechanics tutorial introducing turns, speed portals, and dash orbs.",
        uploadedAt: "2026-01-21",
    },
];

export const mapLeaderboards: Record<string, ScoreEntry[]> = {
    railbreaker: [
        {
            rank: 1,
            playerHandle: "zen",
            accuracy: "99.21%",
            score: "1,254,870",
            pp: 312,
            mods: "+HD",
            playedAt: "2026-02-21",
        },
        {
            rank: 2,
            playerHandle: "lyra",
            accuracy: "98.94%",
            score: "1,249,034",
            pp: 307,
            mods: "+NM",
            playedAt: "2026-02-20",
        },
        {
            rank: 3,
            playerHandle: "atlas",
            accuracy: "98.65%",
            score: "1,242,110",
            pp: 301,
            mods: "+HR",
            playedAt: "2026-02-19",
        },
    ],
    "neon-archive": [
        {
            rank: 1,
            playerHandle: "kairo",
            accuracy: "99.51%",
            score: "1,132,222",
            pp: 264,
            mods: "+NM",
            playedAt: "2026-02-22",
        },
        {
            rank: 2,
            playerHandle: "mira",
            accuracy: "98.89%",
            score: "1,120,100",
            pp: 252,
            mods: "+EZ",
            playedAt: "2026-02-21",
        },
        {
            rank: 3,
            playerHandle: "taro",
            accuracy: "98.12%",
            score: "1,113,803",
            pp: 243,
            mods: "+NM",
            playedAt: "2026-02-20",
        },
    ],
    "first-light": [
        {
            rank: 1,
            playerHandle: "nova",
            accuracy: "100.00%",
            score: "980,000",
            pp: 58,
            mods: "+NM",
            playedAt: "2026-02-16",
        },
        {
            rank: 2,
            playerHandle: "ray",
            accuracy: "99.81%",
            score: "976,100",
            pp: 56,
            mods: "+NM",
            playedAt: "2026-02-16",
        },
        {
            rank: 3,
            playerHandle: "sol",
            accuracy: "99.32%",
            score: "970,820",
            pp: 53,
            mods: "+DT",
            playedAt: "2026-02-17",
        },
    ],
};

export const mapComments: Record<string, CommentEntry[]> = {
    railbreaker: [
        {
            id: "c1",
            authorHandle: "lyra",
            postedAt: "2026-02-21",
            body: "Great sync charting. Last drop could use one more visual warning before the rapid split.",
            votes: 18,
        },
        {
            id: "c2",
            authorHandle: "atlas",
            postedAt: "2026-02-20",
            body: "Nominator stars feel fair; community vote might settle around 5.5.",
            votes: 11,
        },
    ],
    "neon-archive": [
        {
            id: "c3",
            authorHandle: "vex",
            postedAt: "2026-02-22",
            body: "Loved the camera-friendly route design. One corner after chorus needs clearer telegraphing.",
            votes: 9,
        },
    ],
    "first-light": [
        {
            id: "c4",
            authorHandle: "newcomer_37",
            postedAt: "2026-02-18",
            body: "Perfect tutorial pacing. I finally understood dash orb timing.",
            votes: 24,
        },
    ],
};

export const ppLeaderboard: PpEntry[] = [
    {
        rank: 1,
        handle: "zen",
        country: "US",
        pp: 15432,
        accuracy: "98.97%",
        rankedScore: "942,118,211",
        mapsCleared: 938,
    },
    {
        rank: 2,
        handle: "lyra",
        country: "DE",
        pp: 15112,
        accuracy: "98.71%",
        rankedScore: "903,401,833",
        mapsCleared: 887,
    },
    {
        rank: 3,
        handle: "atlas",
        country: "KR",
        pp: 14980,
        accuracy: "98.64%",
        rankedScore: "884,002,153",
        mapsCleared: 864,
    },
    {
        rank: 4,
        handle: "nova",
        country: "CA",
        pp: 14621,
        accuracy: "98.22%",
        rankedScore: "841,221,985",
        mapsCleared: 812,
    },
    {
        rank: 5,
        handle: "kairo",
        country: "JP",
        pp: 14302,
        accuracy: "97.94%",
        rankedScore: "803,111,720",
        mapsCleared: 778,
    },
];

export const profiles: Profile[] = [
    {
        handle: "nova",
        displayName: "Nova",
        bio: "Speed-focused mapper and tester. Building flow maps with strict rhythm readability.",
        country: "CA",
        joinedAt: "2025-07-11",
        followerCount: 1231,
        mapperTier: "Ranked Mapper",
        totalPp: 14621,
        globalRank: 4,
        favoriteMods: ["HD", "HR"],
    },
    {
        handle: "iris",
        displayName: "Iris",
        bio: "Editor artist. I design scenic maps that still feel clean to read at speed.",
        country: "SE",
        joinedAt: "2025-09-03",
        followerCount: 809,
        mapperTier: "Nominator",
        totalPp: 11220,
        globalRank: 19,
        favoriteMods: ["NM", "EZ"],
    },
    {
        handle: "zen",
        displayName: "Zen",
        bio: "Competitive grinder. Chasing consistency and route memory optimization.",
        country: "US",
        joinedAt: "2025-06-14",
        followerCount: 2010,
        mapperTier: "New Mapper",
        totalPp: 15432,
        globalRank: 1,
        favoriteMods: ["HD", "DT"],
    },
];

export function getBeatmap(slug: string): Beatmap | undefined {
    return beatmaps.find((map) => map.slug === slug);
}

export function getProfile(handle: string): Profile | undefined {
    return profiles.find((profile) => profile.handle === handle);
}
