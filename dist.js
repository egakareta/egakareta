import fs from "fs";
import path from "path";
import url from "url";

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));

const start = Date.now();

// Create dist folder if it doesn't exist
const distPath = path.join(__dirname, "dist");
if (!fs.existsSync(distPath)) {
    fs.mkdirSync(distPath);
}

// Copy index.html to dist
fs.copyFileSync(
    path.join(__dirname, "index.html"),
    path.join(distPath, "index.html"),
);

// Copy assets folder to dist
const assetsSrcPath = path.join(__dirname, "assets");
const assetsDestPath = path.join(distPath, "assets");
if (fs.existsSync(assetsDestPath)) {
    fs.rmSync(assetsDestPath, { recursive: true });
}
fs.cpSync(assetsSrcPath, assetsDestPath, { recursive: true });

// Add .nojekyll file to dist
fs.writeFileSync(path.join(distPath, ".nojekyll"), "");

// Add _headers file to dist
const headersContent = `
/*
  Cross-Origin-Opener-Policy: same-origin
  Cross-Origin-Embedder-Policy: require-corp
`;
fs.writeFileSync(path.join(distPath, "_headers"), headersContent.trim());

console.log(`created dist in ${Date.now() - start}ms`);
