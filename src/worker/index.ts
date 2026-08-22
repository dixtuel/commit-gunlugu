import { Worker } from "bullmq";
import IORedis from "ioredis";
import { db } from "@/lib/db";
import { summarizeToEntry } from "@/lib/ai/summarize";
import type { SummarizeJobData } from "@/lib/queue";

const connection = new IORedis(process.env.REDIS_URL ?? "redis://127.0.0.1:6380/2", {
  maxRetriesPerRequest: null,
});

const worker = new Worker<SummarizeJobData>(
  "summarize-entry",
  async (job) => {
    const { projectId, commits, pullRequest } = job.data;

    const draft = await summarizeToEntry({
      commitMessages: commits.map((c) => c.message),
      pullRequestTitle: pullRequest?.title,
      pullRequestBody: pullRequest?.body,
    });

    await db.entry.create({
      data: {
        projectId,
        category: draft.category,
        title: draft.title,
        body: draft.body,
        status: "DRAFT",
        aiGenerated: true,
        sourceCommitShas: commits.map((c) => c.sha),
        sourcePrNumber: pullRequest?.number,
      },
    });

    await db.webhookEvent.updateMany({
      where: { githubDeliveryId: job.data.githubDeliveryId },
      data: { processedAt: new Date() },
    });
  },
  { connection, concurrency: 4 },
);

worker.on("failed", (job, err) => {
  console.error(`[worker] is basarisiz: ${job?.id}`, err);
});

console.log("[worker] summarize-entry kuyrugu dinleniyor");
