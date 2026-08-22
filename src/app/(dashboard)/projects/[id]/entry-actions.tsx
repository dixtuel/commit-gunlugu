"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";

export function EntryActions({ projectId, entryId }: { projectId: string; entryId: string }) {
  const router = useRouter();
  const [pending, setPending] = useState<"publish" | "dismiss" | null>(null);

  async function publish() {
    setPending("publish");
    await fetch(`/api/projects/${projectId}/entries/${entryId}/publish`, { method: "POST" });
    router.refresh();
  }

  async function dismiss() {
    setPending("dismiss");
    await fetch(`/api/projects/${projectId}/entries/${entryId}`, { method: "DELETE" });
    router.refresh();
  }

  return (
    <div className="entryActions">
      <button className="btn btnPrimary" onClick={publish} disabled={pending !== null}>
        {pending === "publish" ? "Yayınlanıyor…" : "Onayla ve yayınla"}
      </button>
      <button className="btn btnDanger" onClick={dismiss} disabled={pending !== null}>
        Reddet
      </button>
    </div>
  );
}
