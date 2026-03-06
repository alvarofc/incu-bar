# Employer Dashboard

Static dashboard for visualizing Tinybird-backed employer reporting data.

## Features

- KPI cards (employees, active providers, remaining %, costs, provider errors)
- Daily trend chart (cost + remaining)
- Provider spend chart
- Employee table with low-remaining highlighting
- Better Auth sign-in / sign-up
- Organization picker from Better Auth organization plugin
- Optional API key fallback for machine troubleshooting

## Run locally

```bash
cd infra/dashboard
python3 -m http.server 5178
```

Open `http://localhost:5178` and set:

- Gateway URL (for example `https://incubar-reporting-gateway.alvaro-112.workers.dev`)
- Better Auth email + password, then sign in
- Organization (loaded from `/v1/dashboard/organizations`)

The page calls `GET /v1/dashboard/overview` on the Worker and includes auth headers.
