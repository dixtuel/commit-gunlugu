import { NextResponse } from "next/server";
import { db } from "@/lib/db";

export async function POST(
  _req: Request,
  { params }: { params: { id: string; entryId: string } },
) {
  const entry = await db.entry.update({
    where: { id: params.entryId, projectId: params.id },
    data: { status: "PUBLISHED", publishedAt: new Date() },
  });

  return NextResponse.json({ entry });
}
