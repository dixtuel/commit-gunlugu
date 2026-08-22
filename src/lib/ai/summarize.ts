import { z } from "zod";

const EntryDraftSchema = z.object({
  category: z.enum(["NEW", "FIX", "IMPROVEMENT"]),
  title: z.string().max(120),
  body: z.string().max(400),
});

export type EntryDraft = z.infer<typeof EntryDraftSchema>;

const SYSTEM_PROMPT = `Sen bir SaaS urununun teknik olmayan musterilerine yonelik changelog editorusun.
Sana git commit mesajlari ve/veya bir pull request aciklamasi verilecek.
Gorevin: jargon icermeyen, 1-2 cumlelik, musteri diline cevrilmis TEK bir degisiklik notu uretmek.
- "fix:", "feat:", "refactor:" gibi on ekleri, dosya adlarini, degisken adlarini asla kullanma.
- Sadece kullaniciyi ilgilendiren sonucu anlat, nasil yapildigini degil.
- category alanini sec: NEW (yeni ozellik), FIX (hata duzeltmesi), IMPROVEMENT (iyilestirme/performans).
- Yalnizca su JSON semasina uyan tek bir nesne don: {"category": "...", "title": "...", "body": "..."}`;

// mikoshi-ai-gateway (LiteLLM, OpenAI /chat/completions uyumlu) uzerinden calisir,
// boylece model secimi merkezi gateway config'inden yonetilebilir.
export async function summarizeToEntry(input: {
  commitMessages: string[];
  pullRequestTitle?: string;
  pullRequestBody?: string | null;
}): Promise<EntryDraft> {
  const userContent = [
    input.pullRequestTitle ? `PR basligi: ${input.pullRequestTitle}` : null,
    input.pullRequestBody ? `PR aciklamasi: ${input.pullRequestBody}` : null,
    `Commit mesajlari:\n${input.commitMessages.map((m) => `- ${m}`).join("\n")}`,
  ]
    .filter(Boolean)
    .join("\n\n");

  const res = await fetch(`${process.env.AI_API_BASE_URL}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${process.env.AI_API_KEY}`,
    },
    body: JSON.stringify({
      model: process.env.AI_MODEL ?? "claude-sonnet-5",
      response_format: { type: "json_object" },
      messages: [
        { role: "system", content: SYSTEM_PROMPT },
        { role: "user", content: userContent },
      ],
    }),
  });

  if (!res.ok) {
    throw new Error(`AI gateway hatasi: ${res.status} ${await res.text()}`);
  }

  const json = (await res.json()) as { choices: { message: { content: string } }[] };
  const raw = json.choices[0]?.message?.content ?? "{}";

  return EntryDraftSchema.parse(JSON.parse(raw));
}
