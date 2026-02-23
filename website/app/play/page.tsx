import GameCanvas from "@/components/GameCanvas";
import { Metadata } from "next";

export const metadata: Metadata = {
    title: "Play | Line Dash",
    description: "Experience Line Dash directly in your browser.",
};

export default function PlayPage() {
    return (
        <main className="relative min-h-screen bg-black">
            <GameCanvas />
        </main>
    );
}
