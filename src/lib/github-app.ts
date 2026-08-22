import { App } from "@octokit/app";

let app: App | undefined;

export function getGitHubApp(): App {
  if (app) return app;

  app = new App({
    appId: process.env.GITHUB_APP_ID!,
    privateKey: process.env.GITHUB_APP_PRIVATE_KEY!.replace(/\\n/g, "\n"),
    oauth: {
      clientId: process.env.GITHUB_APP_CLIENT_ID!,
      clientSecret: process.env.GITHUB_APP_CLIENT_SECRET!,
    },
    webhooks: {
      secret: process.env.GITHUB_WEBHOOK_SECRET!,
    },
  });

  return app;
}

export async function getInstallationOctokit(githubInstallationId: bigint | number) {
  const octokit = await getGitHubApp().getInstallationOctokit(Number(githubInstallationId));
  return octokit;
}

export async function fetchPullRequestDetails(
  githubInstallationId: bigint | number,
  owner: string,
  repo: string,
  pullNumber: number,
) {
  const octokit = await getInstallationOctokit(githubInstallationId);
  const { data } = await octokit.rest.pulls.get({ owner, repo, pull_number: pullNumber });
  return { number: data.number, title: data.title, body: data.body };
}
