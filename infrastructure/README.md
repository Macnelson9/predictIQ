# PredictIQ Infrastructure as Code

This directory contains all infrastructure definitions for PredictIQ using Terraform.

## Structure

```
infrastructure/
├── terraform/
│   ├── main.tf              # Main configuration
│   ├── variables.tf         # Variable definitions
│   ├── outputs.tf           # Output definitions
│   ├── environments/        # Environment-specific configurations
│   │   ├── dev.tfvars
│   │   ├── staging.tfvars
│   │   └── prod.tfvars
│   └── modules/             # Reusable modules
│       ├── vpc/
│       ├── rds/
│       ├── redis/
│       ├── ecs/
│       └── monitoring/
├── ROLLBACK.md              # Rollback procedures
└── README.md                # This file
```

## Prerequisites

- Terraform >= 1.5.0
- AWS CLI configured
- Appropriate AWS IAM permissions
- Access to Terraform state bucket

## Quick Start

### Initialize Terraform

```bash
cd infrastructure/terraform
terraform init
```

### Plan Infrastructure Changes

```bash
# For development environment
terraform plan -var-file="environments/dev.tfvars"

# For staging environment
terraform plan -var-file="environments/staging.tfvars"

# For production environment
terraform plan -var-file="environments/prod.tfvars"
```

### Apply Infrastructure Changes

```bash
# Apply changes (requires approval)
terraform apply -var-file="environments/prod.tfvars"

# Auto-approve (use with caution)
terraform apply -auto-approve -var-file="environments/prod.tfvars"
```

## Environments

### Development (dev)
- Single-node Redis
- Micro RDS instance
- 1 API task
- Minimal resources for testing

### Staging (staging)
- Multi-node Redis (2 nodes)
- Small RDS instance
- 2 API tasks
- Production-like configuration

### Production (prod)
- Multi-node Redis (3 nodes)
- Medium RDS instance
- 3 API tasks
- High availability setup

## Key Components

### VPC Module
- Creates isolated network environment
- Configurable CIDR blocks
- Public and private subnets
- NAT Gateway for private subnet egress

### RDS Module
- PostgreSQL database
- Automated backups
- Multi-AZ deployment (prod)
- Encryption at rest

### Redis Module
- ElastiCache cluster
- Automatic failover
- Parameter group configuration
- Subnet group for VPC placement

### ECS Module
- Fargate launch type
- Application Load Balancer
- Auto-scaling policies
- CloudWatch logging

### Monitoring Module
- CloudWatch dashboards
- SNS alerts
- Log groups
- Metric alarms

## State Management

Terraform state is stored in S3 with:
- Encryption enabled
- Versioning enabled
- DynamoDB table for state locking
- Restricted IAM access

## Deployment Process

1. Create feature branch for infrastructure changes
2. Update Terraform files
3. Run `terraform plan` and review changes
4. Create pull request with plan output
5. After approval, merge to main
6. GitHub Actions automatically applies changes

## Monitoring

Monitor infrastructure health:

```bash
# View Terraform state
terraform show

# Get outputs
terraform output

# Check AWS resources
aws ec2 describe-instances --filters "Name=tag:Project,Values=predictiq"
aws rds describe-db-instances
aws elasticache describe-cache-clusters
```

## Troubleshooting

### State Lock Issues

```bash
# Force unlock (use with caution)
terraform force-unlock <LOCK_ID>
```

### Resource Conflicts

```bash
# Import existing resource
terraform import module.vpc.aws_vpc.main vpc-12345678

# Remove resource from state
terraform state rm module.vpc.aws_vpc.main
```

### Plan Failures

```bash
# Refresh state
terraform refresh

# Validate configuration
terraform validate

# Format configuration
terraform fmt -recursive
```

## Security Best Practices

- Never commit sensitive values to git
- Use AWS Secrets Manager for secrets
- Enable MFA for AWS console access
- Restrict Terraform state bucket access
- Enable CloudTrail for audit logging
- Use IAM roles instead of access keys
- Implement least privilege access

## Cost Optimization

- Use spot instances for non-critical workloads
- Right-size instances based on metrics
- Enable auto-scaling for variable workloads
- Use reserved instances for baseline capacity
- Monitor unused resources

## Support

For infrastructure issues:
1. Check CloudWatch logs
2. Review Terraform state
3. Consult ROLLBACK.md for recovery procedures
4. Contact infrastructure team

## References

- [Terraform Documentation](https://www.terraform.io/docs)
- [AWS Provider Documentation](https://registry.terraform.io/providers/hashicorp/aws/latest/docs)
- [PredictIQ Architecture](../docs/ARCHITECTURE.md)
