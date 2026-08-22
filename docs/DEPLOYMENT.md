# Dağıtım notları (öneri, henüz uygulanmadı)

Bu servis, mevcut Mikoshi VDS'teki `docker-compose.yml` dosyasına aşağıdakine benzer bir
servis olarak eklenebilir. **Bu bir öneri/şablon — dosya henüz canlı compose'a eklenmedi.**

```yaml
  commit-gunlugu-web:
    build: /srv/commit-gunlugu
    restart: unless-stopped
    env_file: /srv/commit-gunlugu/.env
    networks: [internal]

  commit-gunlugu-worker:
    build: /srv/commit-gunlugu
    command: npm run worker
    restart: unless-stopped
    env_file: /srv/commit-gunlugu/.env
    networks: [internal]
```

- **Redis**: mevcut `mikoshi-redis` (127.0.0.1:6380) ayrı bir DB index'i ile paylaşılabilir
  (`REDIS_URL=redis://127.0.0.1:6380/2`), ek bir Redis instance'ı gerekmez.
- **Postgres**: yeni, ayrı bir veritabanı/kullanıcı önerilir (mevcut servislerden izole).
- **Caddy**: `commit-gunlugu.com` ve `*.commit-gunlugu.com` (özel alan adları için) için
  Cloudflare Tunnel üzerinden yeni bir route eklenmesi gerekir.
- **GitHub App secret'ları ve DB parolası** asla bu repoya veya `.env.example` dışına
  commit edilmemeli.
