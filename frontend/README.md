This dashboard is a Next.js app that reads/writes Polymarket bot config files and queries bot runtime data from PostgreSQL.

## Environment

Copy `.env.local.example` to `.env.local` and set:

- `DATABASE_URL`
- `BOT_CONFIG_DIR`
- `AUTH_SECRET`
- `CONFIG_ENCRYPTION_KEY` (base64 encoded 32-byte key, required for encrypted credential writes)
- `SYSTEMD_CONTROL_ENABLED`
- `BOT_SERVICE_NAME` (optional, default: `dextrabot`)
- `AUTH_COOKIE_SECURE` (optional; set `false` for HTTP-only server IP deployments)

`CONFIG_ENCRYPTION_KEY` frontend ve backend'de aynı olmalıdır; aksi halde bot şifreli credentialları çözemez.

`SYSTEMD_CONTROL_ENABLED=true` enables Start/Stop/Restart actions via `sudo systemctl` (host machine only).
If unavailable, UI falls back to manual restart messaging.
For systemd deployments where frontend runs as `dextrabot`, passwordless sudo must allow:
`/usr/bin/systemctl start|stop|restart|is-active <BOT_SERVICE_NAME>`.

`AUTH_SECRET` is also used as login password and should be a random secret with at least 32 characters.

## Run

```bash
cd /home/heathcliff/polymarketbot/frontend
npm run dev
```

Open `http://localhost:3000`.

For server-IP access in production mode:

```bash
cd /home/heathcliff/polymarketbot/frontend
npm run build
npm run start:server
```

Open `http://<SERVER_IP>:3000`.

## Systemd Service (Server IP:3000)

Setup helper:

```bash
cd /home/heathcliff/polymarketbot
./scripts/setup_frontend_service.sh
```

If build-time internet access is restricted (for Google Fonts), use:

```bash
cd /home/heathcliff/polymarketbot
SKIP_FRONTEND_BUILD=true ./scripts/setup_frontend_service.sh
```

This installs:

- `/etc/dextrabot/dextrabot-frontend.env`
- `deploy/systemd/dextrabot-frontend.service` -> `/etc/systemd/system/dextrabot-frontend.service`
- `/etc/sudoers.d/dextrabot-bot-systemctl` (limited `systemctl` permissions for frontend bot control)

Before setup, ensure dev server is not running on port `3000`:

```bash
pkill -f '/home/heathcliff/polymarketbot/frontend/node_modules/.bin/next dev --webpack' || true
```

Important env for HTTP-only deployments:

```bash
AUTH_COOKIE_SECURE=false
```

Because login is mandatory, `AUTH_SECRET` and `CONFIG_ENCRYPTION_KEY` must be set to real values (not `CHANGE_ME`).
`AUTH_SECRET` should be at least 32 characters and must not use a shared or guessable password.

## Dev Process Guard

`npm run dev` now uses a single-process guard script:

- If `next dev` is already running, it exits cleanly instead of starting a second process.
- If `.next/dev/lock` exists but no `next dev` is running, it removes the stale lock and starts.

Useful commands:

```bash
npm run dev:preflight  # checks running dev process + lock consistency
npm run dev            # starts dev if needed, otherwise no-ops safely
npm run dev:restart    # stops existing next dev, clears lock, starts fresh
```

## Troubleshooting

If localhost does not open after file changes and you see:

`Unable to acquire lock at .../.next/dev/lock`

run:

```bash
npm run dev:preflight
npm run dev:restart
```

This is usually caused by multiple `next dev` start attempts or a stale lock after an unclean stop.

---

This project was originally bootstrapped with [`create-next-app`](https://nextjs.org/docs/app/api-reference/cli/create-next-app).

## Getting Started

First, run the development server:

```bash
npm run dev
# or
yarn dev
# or
pnpm dev
# or
bun dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see the result.

You can start editing the page by modifying `app/page.tsx`. The page auto-updates as you edit the file.

This project uses [`next/font`](https://nextjs.org/docs/app/building-your-application/optimizing/fonts) to automatically optimize and load [Geist](https://vercel.com/font), a new font family for Vercel.

## Learn More

To learn more about Next.js, take a look at the following resources:

- [Next.js Documentation](https://nextjs.org/docs) - learn about Next.js features and API.
- [Learn Next.js](https://nextjs.org/learn) - an interactive Next.js tutorial.

You can check out [the Next.js GitHub repository](https://github.com/vercel/next.js) - your feedback and contributions are welcome!

## Deploy on Vercel

The easiest way to deploy your Next.js app is to use the [Vercel Platform](https://vercel.com/new?utm_medium=default-template&filter=next.js&utm_source=create-next-app&utm_campaign=create-next-app-readme) from the creators of Next.js.

Check out our [Next.js deployment documentation](https://nextjs.org/docs/app/building-your-application/deploying) for more details.
