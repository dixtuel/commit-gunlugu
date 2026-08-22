import { NextRequest, NextResponse } from "next/server";
import { z } from "zod";
import { db } from "@/lib/db";

const UpdateSchema = z.object({
  title: z.string().min(1).max(120).optional(),
  body: z.string().min(1).max(400).optional(),
  category: z.enum(["NEW", "FIX", "IMPROVEMENT"]).optional(),
});

export async function PATCH(
  req: NextRequest,
  { params }: { params: { id: string; entryId: string } },
) {
  const body = UpdateSchema.parse(await req.json());

  const entry = await db.entry.update({
    where: { id: params.entryId, projectId: params.id },
    data: body,
  });

  return NextResponse.json({ entry });
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: { id: string; entryId: string } },
) {
  await db.entry.update({
    where: { id: params.entryId, projectId: params.id },
    data: { status: "DISMISSED" },
  });

  return NextResponse.json({ status: "reddedildi" });
}
