import { NextRequest, NextResponse } from "next/server";
import { db } from "@/lib/db";
import { getSessionOrgId } from "@/lib/session";

// GitHub App kurulumu tamamlaninca GitHub'in yonlendirdigi callback.
// ?installation_id=...&setup_action=install
export async function GET(req: NextRequest) {
  const installationId = req.nextUrl.searchParams.get("installation_id");
  const setupAction = req.nextUrl.searchParams.get("setup_action");

  if (!installationId || setupAction !== "install") {
    return NextResponse.redirect(new URL("/onboarding?hata=kurulum-iptal", req.url));
  }

  const organizationId = await getSessionOrgId();
  if (!organizationId) {
    return NextResponse.redirect(new URL("/giris", req.url));
  }

  await db.installation.upsert({
    where: { githubInstallationId: BigInt(installationId) },
    update: { organizationId },
    create: {
      githubInstallationId: BigInt(installationId),
      githubAccountLogin: "",
      organizationId,
    },
  });

  return NextResponse.redirect(new URL("/onboarding/repolar", req.url));
}
