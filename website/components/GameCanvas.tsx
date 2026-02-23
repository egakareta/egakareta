"use client";

import { useEffect, useRef, useState } from "react";

export default function GameCanvas() {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        const installWebGpuLimitsCompat = () => {
            const gpu = (navigator as any).gpu;
            if (!gpu) return;
            const proto = Object.getPrototypeOf(
                (globalThis as any).GPUAdapter?.prototype ?? {},
            );
            const adapterProto =
                (globalThis as any).GPUAdapter?.prototype ?? proto;
            if (!adapterProto || adapterProto.__lineDashRequestDevicePatched)
                return;

            const originalRequestDevice = adapterProto.requestDevice;
            if (typeof originalRequestDevice !== "function") return;

            const limitNameMap: Record<string, string> = {
                maxInterStageShaderComponents: "maxInterStageShaderVariables",
            };

            adapterProto.requestDevice = function patchedRequestDevice(
                descriptor: any = {},
            ) {
                const limits = descriptor?.requiredLimits;
                if (!limits || typeof limits !== "object") {
                    return originalRequestDevice.call(this, descriptor);
                }

                const normalizedLimits: any = {};
                for (const [key, value] of Object.entries(limits)) {
                    const mappedKey = limitNameMap[key] ?? key;
                    normalizedLimits[mappedKey] = value;
                }

                return originalRequestDevice.call(this, {
                    ...descriptor,
                    requiredLimits: normalizedLimits,
                });
            };

            Object.defineProperty(
                adapterProto,
                "__lineDashRequestDevicePatched",
                {
                    value: true,
                    enumerable: false,
                    configurable: false,
                    writable: false,
                },
            );
        };

        async function initGame() {
            try {
                installWebGpuLimitsCompat();

                // Dynamically import the WASM module from the public directory at runtime
                const { default: init, run_game } = await import(
                    // @ts-ignore
                    /* webpackIgnore: true */ "/pkg/line_dash_lib.js"
                );

                await init();

                if (canvasRef.current) {
                    await run_game("gameCanvas");
                }
                setLoading(false);
            } catch (err: any) {
                console.error("Failed to load game:", err);
                setError(err.message || "Unknown error");
                setLoading(false);
            }
        }

        initGame();
    }, []);

    return (
        <div className="fixed inset-0 z-50 bg-black">
            {loading && (
                <div className="absolute inset-0 flex flex-col items-center justify-center text-white space-y-4">
                    <div className="text-2xl font-display tracking-widest animate-pulse">
                        LOADING ENGINE
                    </div>
                    <div className="w-48 h-1 bg-slate-800 rounded-full overflow-hidden">
                        <div
                            className="h-full bg-cyan-400 animate-[loading_2s_ease-in-out_infinite]"
                            style={{ width: "30%" }}
                        ></div>
                    </div>
                </div>
            )}
            {error && (
                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-red-950/80 border border-red-500 p-6 rounded-xl text-white z-60 max-w-md">
                    <h2 className="text-xl font-bold mb-2">Engine Error</h2>
                    <p className="text-red-200 opacity-90">{error}</p>
                    <button
                        onClick={() => window.location.reload()}
                        className="mt-4 px-4 py-2 bg-red-500 hover:bg-red-400 rounded-lg transition-colors"
                    >
                        Retry
                    </button>
                </div>
            )}
            <canvas
                ref={canvasRef}
                className="block w-full h-full outline-none"
                id="gameCanvas"
            />
        </div>
    );
}
