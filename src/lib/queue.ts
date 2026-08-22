import { Queue } from "bullmq";
import IORedis from "ioredis";

const connection = new IORedis(process.env.REDIS_URL ?? "redis://127.0.0.1:6380/2", {
  maxRetriesPerRequest: null,
});

export type SummarizeJobData = {
  projectId: string;
  githubDeliveryId: string;
  commits: { sha: string; message: string; url: string }[];
  pullRequest?: { number: number; title: string; body: string | null };
};

export const summarizeQueue = new Queue<SummarizeJobData>("summarize-entry", { connection });
