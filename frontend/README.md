# Dextrabot Dashboard

This is the Next.js dashboard for Dextrabot. It reads and writes bot configuration, queries PostgreSQL runtime data, manages trade-flow definitions, and exposes operational controls for the backend service.

## Environment

Create `frontend/.env.local` for local development:

```bash
DATABASE_URL=postgres://dextrabot_app:<password>@127.0.0.1:5432/dextrabot
BOT_CONFIG_DIR=../config
AUTH_SECRET=<strong-secret-at-least-32-characters>
CONFIG_ENCRYPTION_KEY=<base64-encoded-32-byte-key>
SYSTEMD_CONTROL_ENABLED=false
AUTH_COOKIE_SECURE=false
```

`CONFIG_ENCRYPTION_KEY` must match the backend key so encrypted exchange, claim, and Telegram settings can be read by both processes.

## Development

```bash
npm install
npm run dev
```

Open `http://localhost:3000`.

The dev command uses `scripts/dev-single.sh` to avoid starting multiple `next dev` processes against the same `.next/dev/lock`.

Useful commands:

```bash
npm run dev:preflight
npm run dev:restart
npm run lint
npm run build
npm run test:unit
```

## Production

Build and run directly:

```bash
npm run build
npm run start:server
```

Or install the systemd service from the repository root:

```bash
./scripts/setup_frontend_service.sh
```

For production redeploys, prefer the setup script. It builds the bundle, makes
`.next` readable by the `dextrabot` service user, and restarts
`dextrabot-frontend`; if you run `npm run build` manually, repeat the permission
fix and service restart before using the dashboard.

When `SYSTEMD_CONTROL_ENABLED=true`, the service user needs passwordless permission for:

```text
/usr/bin/systemctl start|stop|restart|is-active <BOT_SERVICE_NAME>
```

The setup script writes a narrow sudoers rule for the configured bot service.
