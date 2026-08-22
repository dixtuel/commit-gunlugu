import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Commit Günlüğü",
  description: "GitHub commit'lerinizi otomatik, beyaz etiketli bir changelog widget'ına çevirir.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="tr">
      <body>{children}</body>
    </html>
  );
}
