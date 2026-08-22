import { NextResponse } from "next/server";
import { db } from "@/lib/db";

// Genel (auth'suz), CORS'a acik uc nokta - musteri sitesine gomulen widget bunu okur.
export async function GET(_req: Request, { params }: { params: { widgetKey: string } }) {
  const project = await db.project.findUnique({
    where: { widgetKey: params.widgetKey },
    select: {
      name: true,
      brandName: true,
      brandColor: true,
      brandLogoUrl: true,
      hideBranding: true,
      slug: true,
      entries: {
        where: { status: "PUBLISHED" },
        orderBy: { publishedAt: "desc" },
        take: 20,
        select: { id: true, category: true, title: true, body: true, publishedAt: true },
      },
    },
  });

  if (!project) {
    return NextResponse.json({ error: "proje bulunamadi" }, { status: 404 });
  }

  return NextResponse.json({
    brand: {
      name: project.brandName ?? project.name,
      color: project.brandColor,
      logoUrl: project.brandLogoUrl,
      showPoweredBy: !project.hideBranding,
    },
    changelogUrl: `${process.env.APP_BASE_URL}/c/${project.slug}`,
    entries: project.entries,
  });
}
