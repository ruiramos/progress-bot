# GitHub Actions Setup Guide

This directory contains GitHub Actions workflows for CI/CD.

## Workflows

### build-and-push.yml
Builds Docker image and pushes to Google Cloud Artifact Registry.

**Triggers:**
- Push to main/master branch
- Git tags (v*)
- Pull requests
- Manual workflow dispatch

**What it does:**
1. Builds multi-arch Docker image
2. Pushes to Google Artifact Registry
3. Tags images appropriately (latest, semver, SHA)
4. Optional: Deploys to GKE (disabled by default)

## Setup Instructions

### 1. Create Google Cloud Artifact Registry

```bash
# Set variables
PROJECT_ID="your-gcp-project-id"
REGION="us-central1"  # Change as needed
REPO_NAME="progress-bot-repo"  # Your repository name

# Create repository
gcloud artifacts repositories create ${REPO_NAME} \
  --repository-format=docker \
  --location=${REGION} \
  --project=${PROJECT_ID} \
  --description="Docker images for Progress Bot"

# Verify
gcloud artifacts repositories list --project=${PROJECT_ID}
```

### 2. Configure Workload Identity Federation (Recommended)

Workload Identity Federation is more secure than service account keys.

```bash
# Set variables
PROJECT_ID="your-gcp-project-id"
PROJECT_NUMBER=$(gcloud projects describe ${PROJECT_ID} --format='value(projectNumber)')
POOL_NAME="github-pool"
PROVIDER_NAME="github-provider"
SERVICE_ACCOUNT_NAME="github-actions-sa"
GITHUB_REPO="your-github-username/progress-bot"  # e.g., "octocat/my-repo"

# Enable required APIs
gcloud services enable iamcredentials.googleapis.com \
  --project=${PROJECT_ID}

# Create service account
gcloud iam service-accounts create ${SERVICE_ACCOUNT_NAME} \
  --display-name="GitHub Actions Service Account" \
  --project=${PROJECT_ID}

# Grant permissions to service account
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
  --member="serviceAccount:${SERVICE_ACCOUNT_NAME}@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.writer"

# Optional: Grant GKE access for deployment
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
  --member="serviceAccount:${SERVICE_ACCOUNT_NAME}@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/container.developer"

# Create Workload Identity Pool
gcloud iam workload-identity-pools create ${POOL_NAME} \
  --location="global" \
  --project=${PROJECT_ID}

# Create Workload Identity Provider
gcloud iam workload-identity-pools providers create-oidc ${PROVIDER_NAME} \
  --location="global" \
  --workload-identity-pool=${POOL_NAME} \
  --issuer-uri="https://token.actions.githubusercontent.com" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" \
  --project=${PROJECT_ID}

# Allow GitHub repo to impersonate service account
gcloud iam service-accounts add-iam-policy-binding \
  ${SERVICE_ACCOUNT_NAME}@${PROJECT_ID}.iam.gserviceaccount.com \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/${POOL_NAME}/attribute.repository/${GITHUB_REPO}" \
  --project=${PROJECT_ID}

# Get the Workload Identity Provider resource name
gcloud iam workload-identity-pools providers describe ${PROVIDER_NAME} \
  --location="global" \
  --workload-identity-pool=${POOL_NAME} \
  --project=${PROJECT_ID} \
  --format="value(name)"
```

### 3. Alternative: Service Account Key (Less Secure)

If you can't use Workload Identity Federation:

```bash
# Create service account
gcloud iam service-accounts create github-actions \
  --display-name="GitHub Actions" \
  --project=${PROJECT_ID}

# Grant permissions
gcloud projects add-iam-policy-binding ${PROJECT_ID} \
  --member="serviceAccount:github-actions@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.writer"

# Create and download key
gcloud iam service-accounts keys create key.json \
  --iam-account=github-actions@${PROJECT_ID}.iam.gserviceaccount.com

# Copy the contents of key.json for GitHub secrets
cat key.json
```

### 4. Configure GitHub Secrets

Go to your GitHub repository → Settings → Secrets and variables → Actions

**Required secrets:**

1. **GCP_PROJECT_ID**
   - Your GCP project ID
   - Example: `my-project-123`

2. **For Workload Identity Federation (recommended):**
   - **GCP_WORKLOAD_IDENTITY_PROVIDER**
     - The full resource name from step 2
     - Example: `projects/123456789/locations/global/workloadIdentityPools/github-pool/providers/github-provider`
   - **GCP_SERVICE_ACCOUNT**
     - Service account email
     - Example: `github-actions-sa@my-project-123.iam.gserviceaccount.com`

3. **For Service Account Key (alternative):**
   - **GCP_SA_KEY**
     - Contents of the key.json file from step 3
     - Paste the entire JSON content

**Optional secrets (for automatic deployment):**

4. **GKE_CLUSTER_NAME**
   - Your GKE cluster name
   - Example: `progress-bot-cluster`

5. **GKE_CLUSTER_REGION**
   - Your GKE cluster region
   - Example: `us-central1`

### 5. Update Workflow Configuration

Edit `.github/workflows/build-and-push.yml`:

```yaml
env:
  GAR_LOCATION: us-central1  # Your GAR region
  GAR_REPOSITORY: progress-bot-repo  # Your repository name from step 1
  IMAGE_NAME: progress-bot
  GCP_PROJECT_ID: ${{ secrets.GCP_PROJECT_ID }}
```

### 6. Update Kubernetes Manifests

Update `k8s/deployment.yaml` and `k8s/reminders-cronjob.yaml` with your GAR image path:

```yaml
image: us-central1-docker.pkg.dev/YOUR_PROJECT_ID/progress-bot-repo/progress-bot:latest
```

Or use the Makefile:
```bash
make k8s-deploy REGISTRY=us-central1-docker.pkg.dev/YOUR_PROJECT_ID/progress-bot-repo
```

### 7. Enable Automatic Deployment (Optional)

To enable automatic deployment to GKE after successful builds:

1. Ensure GKE secrets are configured (step 4)
2. In `.github/workflows/build-and-push.yml`, find the `deploy` job
3. Remove or change this line: `if: false` to `if: true`

## Testing the Workflow

### Test with workflow_dispatch
1. Go to GitHub → Actions tab
2. Select "Build and Push to GAR" workflow
3. Click "Run workflow"
4. Select branch and click "Run workflow"

### Test with a push
```bash
git add .
git commit -m "Test CI/CD pipeline"
git push origin main
```

### Test with a tag
```bash
git tag v1.0.0
git push origin v1.0.0
```

## Workflow Features

### Automatic Tagging
- **latest** - Always points to the latest main/master build
- **SHA** - Tagged with git commit SHA (e.g., `main-abc1234`)
- **branch** - Tagged with branch name
- **semver** - Git tags like `v1.0.0` become `1.0.0`, `1.0`, `1`

### Build Cache
Uses GitHub Actions cache to speed up builds:
- Layer caching between builds
- Significantly faster subsequent builds

### Multi-architecture
Currently builds for `linux/amd64`. Add more platforms if needed:
```yaml
platforms: linux/amd64,linux/arm64
```

## Troubleshooting

### Authentication Failed
- Verify service account has `artifactregistry.writer` role
- Check project ID is correct
- For Workload Identity: Ensure repository name matches exactly

### Image Push Failed
- Verify GAR repository exists
- Check region matches
- Ensure service account has permissions

### Deployment Failed
- Verify GKE credentials are correct
- Check kubectl can access cluster
- Ensure service account has `container.developer` role

### View workflow logs
GitHub → Actions → Select workflow run → View logs

## Security Best Practices

1. ✅ Use Workload Identity Federation (no keys to manage)
2. ✅ Never commit service account keys to git
3. ✅ Use least privilege permissions
4. ✅ Enable branch protection on main/master
5. ✅ Review pull request builds before merging
6. ✅ Use signed commits
7. ✅ Enable vulnerability scanning in GAR

## Additional Resources

- [Workload Identity Federation](https://cloud.google.com/iam/docs/workload-identity-federation)
- [Artifact Registry Documentation](https://cloud.google.com/artifact-registry/docs)
- [GitHub Actions with Google Cloud](https://github.com/google-github-actions)
- [Docker Build Push Action](https://github.com/docker/build-push-action)
