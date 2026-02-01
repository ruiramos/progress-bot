# progress-bot
A progress / standup bot for Slack

### Notes
  App is part of Amigos de Coimbra Slack Group
 - To install the development version: https://slack.com/oauth/authorize?client_id=2168708159.764740633012&scope=bot%2Cusers%3Aread%2Cusers.profile%3Aread%2Ccommands
 - Need to change urls on events subscriptions and slack commands individually
 - oauth & permissions has the redirect urls - it will default to the first one so leave the param out of the add to slack request

## Development

### Local Setup
```bash
# Start PostgreSQL
docker-compose up -d

# Run migrations
diesel migration run

# Start the web server
cargo run

# Start the reminders daemon (in a separate terminal)
cargo run --bin reminders
```

## Deployment

### Kubernetes with GitHub Actions (Recommended)
The project includes automated CI/CD with GitHub Actions that builds and pushes to Google Artifact Registry.

**Setup:**
1. Follow [.github/workflows/README.md](.github/workflows/README.md) to configure GitHub Actions
2. Push to main branch or create a tag to trigger build
3. Images automatically pushed to GAR

**Deploy:**
```bash
# Update image references in k8s manifests
make k8s-update-image \
  GAR_REGION=us-central1 \
  GCP_PROJECT_ID=your-project \
  GAR_REPO=your-repo

# Create secrets
kubectl create secret generic progress-bot-secrets \
  --from-literal=database-url='postgres://...' \
  --from-literal=client-id='...' \
  --from-literal=client-secret='...'

# Deploy
kubectl apply -f k8s/
```

See [k8s/README.md](k8s/README.md) for detailed instructions.

### Manual Kubernetes Deployment
```bash
# Build and push image
./build-and-push.sh us-central1-docker.pkg.dev/PROJECT_ID/REPO/progress-bot v1.0.0

# Or using Make
make docker-push \
  GAR_REGION=us-central1 \
  GCP_PROJECT_ID=your-project \
  GAR_REPO=your-repo \
  TAG=v1.0.0

# Deploy
make k8s-deploy
```

### Heroku
The included `Procfile` supports Heroku deployment:
```bash
heroku create
heroku addons:create heroku-postgresql
heroku config:set CLIENT_ID=your-client-id
heroku config:set CLIENT_SECRET=your-client-secret
git push heroku master
```

### Docker
```bash
# Build image
docker build -t progress-bot .

# Run container
docker run -p 8800:8800 \
  -e DATABASE_URL='postgres://...' \
  -e CLIENT_ID='...' \
  -e CLIENT_SECRET='...' \
  progress-bot
```
