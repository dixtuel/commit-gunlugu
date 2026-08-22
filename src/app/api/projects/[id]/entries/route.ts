import { NextRequest, NextResponse } from "next/server";
import { db } from "@/lib/db";

export async function GET(req: NextRequest, { params }: { params: { id: string } }) {
  const status = req.nextUrl.searchParams.get("status") ?? undefined;

  const entries = await db.entry.findMany({
    where: { projectId: params.id, ...(status ? { status: status as never } : {}) },
    orderBy: { createdAt: "desc" },
  });

  return NextResponse.json({ entries });
}
