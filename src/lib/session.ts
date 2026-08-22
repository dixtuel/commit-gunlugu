import { getServerSession } from "next-auth";
import { authOptions } from "@/lib/auth";
import { db } from "@/lib/db";

// MVP: kullanicinin ilk (tek) organizasyonunu doner. Coklu-org secimi v2 kapsaminda.
export async function getSessionOrgId(): Promise<string | null> {
  const session = await getServerSession(authOptions);
  if (!session?.user?.email) return null;

  const membership = await db.orgMember.findFirst({
    where: { user: { email: session.user.email } },
    select: { organizationId: true },
  });

  return membership?.organizationId ?? null;
}
