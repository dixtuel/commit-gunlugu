import { build } from "esbuild";

// Hedef: <15KB minified, framework yok. Musteri sitesine performans yuku bindirmemek
// (bkz. docs/PLAN.md §5) urunun temel farklilastiricilarindan biri.
await build({
  entryPoints: ["widget/src/index.ts"],
  bundle: true,
  minify: true,
  target: "es2018",
  outfile: "widget/dist/widget.js",
});

console.log("widget/dist/widget.js yazildi");
