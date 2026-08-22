import type { NextAuthOptions } from "next-auth";
import GitHubProvider from "next-auth/providers/github";
import { db } from "@/lib/db";

// Dashboard girisi icin ayri, kapsam dar bir GitHub OAuth App kullanilir
// (repo erisimi GitHub App kurulumundan gelir, bu sadece kimlik dogrulamadir).
export const authOptions: NextAuthOptions = {
  providers: [
    GitHubProvider({
      clientId: process.env.GITHUB_APP_CLIENT_ID!,
      clientSecret: process.env.GITHUB_APP_CLIENT_SECRET!,
    }),
  ],
  callbacks: {
    async signIn({ user, profile }) {
      const githubId = String((profile as { id?: number })?.id ?? "");
      if (!githubId) return false;

      await db.user.upsert({
        where: { githubId },
        update: { name: user.name, email: user.email, avatarUrl: user.image },
        create: { githubId, name: user.name, email: user.email, avatarUrl: user.image },
      });

      return true;
    },
    async session({ session }) {
      return session;
    },
  },
  pages: {
    signIn: "/giris",
  },
};
