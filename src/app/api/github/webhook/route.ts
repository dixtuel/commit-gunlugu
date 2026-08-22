import { NextRequest, NextResponse } from "next/server";
import { verify } from "@octokit/webhooks-methods";
import { db } from "@/lib/db";
import { summarizeQueue } from "@/lib/queue";
import { fetchPullRequestDetails } from "@/lib/github-app";

// GitHub'in gonderdigi push ve pull_request (merged) olaylarini isler.
// Imza dogrulamasi + delivery id ile idempotency sagliyor, agir isi (AI cagrisi) kuyruga devrediyor.
export async function POST(req: NextRequest) {
  const rawBody = await req.text();
  const signature = req.headers.get("x-hub-signature-256") ?? "";
  const deliveryId = req.headers.get("x-github-delivery") ?? "";
  const eventType = req.headers.get("x-github-event") ?? "";

  const isValid = await verify(process.env.GITHUB_WEBHOOK_SECRET!, rawBody, signature);
  if (!isValid) {
    return NextResponse.json({ error: "gecersiz imza" }, { status: 401 });
  }

  const existing = await db.webhookEvent.findUnique({ where: { githubDeliveryId: deliveryId } });
  if (existing) {
    return NextResponse.json({ status: "zaten islendi" });
  }

  const payload = JSON.parse(rawBody);
  const repoFullName: string | undefined = payload.repository?.full_name;

  const project = repoFullName
    ? await db.project.findFirst({ where: { githubRepoFullName: repoFullName } })
    : null;

  await db.webhookEvent.create({
    data: {
      githubDeliveryId: deliveryId,
      eventType,
      payload,
      projectId: project?.id,
    },
  });

  if (!project) {
    return NextResponse.json({ status: "eslesen proje yok, kaydedildi" });
  }

  if (eventType === "push" && Array.isArray(payload.commits) && payload.commits.length > 0) {
    await summarizeQueue.add("push", {
      projectId: project.id,
      githubDeliveryId: deliveryId,
      commits: payload.commits.map((c: { id: string; message: string; url: string }) => ({
        sha: c.id,
        message: c.message,
        url: c.url,
      })),
    });
  }

  if (eventType === "pull_request" && payload.action === "closed" && payload.pull_request?.merged) {
    const [owner, repo] = repoFullName!.split("/");
    const pr = await fetchPullRequestDetails(
      payload.installation.id,
      owner!,
      repo!,
      payload.pull_request.number,
    );

    await summarizeQueue.add("pull_request", {
      projectId: project.id,
      githubDeliveryId: deliveryId,
      commits: [],
      pullRequest: pr,
    });
  }

  return NextResponse.json({ status: "kuyruga alindi" });
}
