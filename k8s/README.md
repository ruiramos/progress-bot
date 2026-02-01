# Kubernetes Deployment Guide

This directory contains Kubernetes manifests for deploying Progress Bot.

## Prerequisites

- Kubernetes cluster (1.19+)
- kubectl configured to access your cluster
- PostgreSQL database (can be in-cluster or external)
- Docker registry to store the image
- Slack app credentials (CLIENT_ID and CLIENT_SECRET)

## Quick Start

### 1. Build and Push Docker Image

**Option A: Using GitHub Actions (Recommended)**

Push to main branch or create a tag to trigger automatic build:
```bash
git push origin main
# or
git tag v1.0.0 && git push origin v1.0.0
```

See [../.github/workflows/README.md](../.github/workflows/README.md) for setup instructions.

**Option B: Manual Build**

```bash
# Set your GAR details
REGION="us-central1"
PROJECT_ID="your-project-id"
REPO_NAME="your-gar-repo"

# Build the image
docker build -t ${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO_NAME}/progress-bot:latest .

# Authenticate to GAR
gcloud auth configure-docker ${REGION}-docker.pkg.dev

# Push to your registry
docker push ${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO_NAME}/progress-bot:latest
```

### 2. Create Secrets

**Option A: Using kubectl**
```bash
kubectl create secret generic progress-bot-secrets \
  --from-literal=database-url='postgres://user:password@host:5432/database' \
  --from-literal=client-id='your-slack-client-id' \
  --from-literal=client-secret='your-slack-client-secret'
```

**Option B: Using secrets.yaml**
```bash
# Copy the example and edit with your values
cp secrets.yaml.example secrets.yaml
# IMPORTANT: Add secrets.yaml to .gitignore!
echo "k8s/secrets.yaml" >> .gitignore

# Apply the secret
kubectl apply -f secrets.yaml
```

### 3. Update Image References

Edit the following files and replace the placeholders with your actual values:
- `deployment.yaml` - Replace `REGION`, `PROJECT_ID`, and `PLACEHOLDER_GAR_REPO`
- `reminders-cronjob.yaml` - Replace `REGION`, `PROJECT_ID`, and `PLACEHOLDER_GAR_REPO`

Example:
```yaml
# Before
image: REGION-docker.pkg.dev/PROJECT_ID/PLACEHOLDER_GAR_REPO/progress-bot:latest

# After
image: us-central1-docker.pkg.dev/my-project-123/progress-bot-repo/progress-bot:latest
```

Or use sed:
```bash
REGION="us-central1"
PROJECT_ID="my-project-123"
REPO_NAME="progress-bot-repo"

sed -i "s|REGION-docker.pkg.dev/PROJECT_ID/PLACEHOLDER_GAR_REPO|${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO_NAME}|g" \
  k8s/deployment.yaml k8s/reminders-cronjob.yaml
```

### 4. Deploy to Kubernetes

```bash
# Deploy all manifests
kubectl apply -f k8s/

# Or deploy individually:
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/service.yaml
kubectl apply -f k8s/reminders-cronjob.yaml
kubectl apply -f k8s/ingress.yaml  # Optional, if using ingress
```

### 5. Verify Deployment

```bash
# Check pods
kubectl get pods -l app=progress-bot

# Check service
kubectl get svc progress-bot

# View logs
kubectl logs -l app=progress-bot --tail=100 -f

# Check reminders cronjob
kubectl get cronjobs
```

## Components

### deployment.yaml
- Main web server deployment
- 2 replicas for high availability
- Health checks configured
- Runs migrations via init container
- Resource limits set

### service.yaml
- ClusterIP service exposing the web server
- Maps port 80 to container port 8800

### reminders-cronjob.yaml
- CronJob running the reminders daemon
- Runs hourly by default (adjust schedule as needed)
- Prevents concurrent executions

### ingress.yaml (Optional)
- Exposes the service externally
- Configure with your domain and TLS settings
- Requires an ingress controller (nginx, traefik, etc.)

### secrets.yaml.example
- Template for creating secrets
- **Never commit actual secrets to git!**

## Configuration

### Environment Variables

All configuration is done via environment variables in the deployment:

- `DATABASE_URL` - PostgreSQL connection string
- `CLIENT_ID` - Slack app client ID
- `CLIENT_SECRET` - Slack app client secret
- `PORT` - HTTP port (default: 8800)
- `ROCKET_ENV` - Rocket environment (set to "production")

### Resource Limits

Adjust resource requests/limits in `deployment.yaml` based on your usage:

```yaml
resources:
  requests:
    memory: "128Mi"
    cpu: "100m"
  limits:
    memory: "512Mi"
    cpu: "500m"
```

### Scaling

Scale the deployment:
```bash
kubectl scale deployment progress-bot --replicas=3
```

Or update replicas in `deployment.yaml` and reapply.

### Reminders Schedule

The reminders cronjob runs hourly by default. To change the schedule, edit `reminders-cronjob.yaml`:

```yaml
spec:
  # Cron format: minute hour day month weekday
  schedule: "0 * * * *"  # Every hour
  # schedule: "0 9 * * 1-5"  # 9am weekdays only
  # schedule: "*/30 * * * *"  # Every 30 minutes
```

## Database Setup

### Option 1: External PostgreSQL

Use an external database (AWS RDS, Google Cloud SQL, etc.):
1. Create the database
2. Set `DATABASE_URL` in secrets to point to external database
3. Migrations will run automatically on deployment

### Option 2: In-Cluster PostgreSQL

Deploy PostgreSQL in the cluster:

```bash
# Example using Bitnami PostgreSQL Helm chart
helm repo add bitnami https://charts.bitnami.com/bitnami
helm install postgres bitnami/postgresql \
  --set auth.database=progress_bot \
  --set auth.username=progress_bot \
  --set auth.password=your-password

# Set DATABASE_URL to:
# postgres://progress_bot:your-password@postgres-postgresql:5432/progress_bot
```

## Monitoring

### Logs

```bash
# Web server logs
kubectl logs -l app=progress-bot -f

# Reminders logs
kubectl logs -l app=progress-bot-reminders -f

# Recent failed jobs
kubectl get jobs --field-selector status.successful=0
```

### Health Checks

The deployment includes liveness and readiness probes:
- Liveness: Restarts pod if unhealthy for 30s
- Readiness: Removes pod from service if not ready

Check probe status:
```bash
kubectl describe pod <pod-name>
```

## Troubleshooting

### Pods not starting

```bash
# Check pod status
kubectl get pods -l app=progress-bot

# Describe pod for events
kubectl describe pod <pod-name>

# Check logs
kubectl logs <pod-name>

# Check init container logs (migrations)
kubectl logs <pod-name> -c migrations
```

### Database connection issues

```bash
# Verify secret exists
kubectl get secret progress-bot-secrets

# Check DATABASE_URL is correct
kubectl get secret progress-bot-secrets -o jsonpath='{.data.database-url}' | base64 -d

# Test connection from a debug pod
kubectl run -it --rm debug --image=postgres:15 --restart=Never -- \
  psql "postgres://user:pass@host:5432/database"
```

### Migration issues

```bash
# Check init container logs
kubectl logs <pod-name> -c migrations

# Manually run migrations
kubectl exec -it <pod-name> -- diesel migration run

# Check migration status
kubectl exec -it <pod-name> -- diesel migration list
```

### Slack webhook issues

Ensure your ingress/service is accessible from Slack:
1. Slack needs to reach your `/` endpoint for events
2. Check ingress configuration and DNS
3. Verify TLS certificates if using HTTPS
4. Update Slack app Event Subscriptions URL

## Security Best Practices

1. **Never commit secrets** - Use secret management tools
2. **Run as non-root** - Already configured in Dockerfile
3. **Use network policies** - Restrict pod-to-pod communication
4. **Enable TLS** - Use cert-manager for automatic certificates
5. **Scan images** - Use tools like Trivy or Snyk
6. **Limit resources** - Prevent resource exhaustion
7. **Use read-only root filesystem** - Add to deployment if possible

## Cleanup

Remove all resources:

```bash
kubectl delete -f k8s/
kubectl delete secret progress-bot-secrets
```

## Additional Resources

- [Kubernetes Documentation](https://kubernetes.io/docs/)
- [Rocket Configuration](https://rocket.rs/v0.5/guide/configuration/)
- [Diesel Migrations](https://diesel.rs/guides/getting-started.html)
- [Slack Event Subscriptions](https://api.slack.com/events-api)
