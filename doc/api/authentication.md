# Authentication

This document describes the authentication mechanism used for integrating with the CloudMon Status Dashboard.

## Overview

The metrics-processor uses JWT (JSON Web Token) authentication when reporting component status to the status-dashboard API. This is specifically used by the `cloudmon-metrics-reporter` component to securely communicate health status updates.

## JWT Token Mechanism

### Token Generation

JWT tokens are generated using the HMAC-SHA256 algorithm with a shared secret key.

**Algorithm:** `HS256` (HMAC with SHA-256)

**Token Structure:**

The JWT token contains a simple claim structure:

```json
{
  "stackmon": "dummy"
}
```

**Signing Process:**

1. The shared secret is loaded from configuration (`status_dashboard.jwt_secret`)
2. An HMAC-SHA256 key is created from the secret bytes
3. Claims are signed with the key to produce the JWT token
4. The token is included in the `Authorization` header as a Bearer token

### Configuration

Authentication is configured in the `status_dashboard` section of the configuration file:

```yaml
status_dashboard:
  url: https://status-dashboard.example.com
  secret: your-shared-secret-key
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | Yes | Status dashboard base URL |
| `secret` | string | No | JWT signing secret. If not provided, requests are sent without authentication |

### Environment Variable Override

The secret can also be set via environment variable:

```bash
export MP_STATUS_DASHBOARD__JWT_SECRET="your-shared-secret-key"
```

Environment variables are merged with the configuration file, with environment variables taking precedence.

## Token Usage

### Authorization Header

When making requests to the status-dashboard, the JWT token is included in the HTTP `Authorization` header using the Bearer scheme:

```
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdGFja21vbiI6ImR1bW15In0.<signature>
```

### Request Flow

1. **Configuration Load:** The reporter reads the `status_dashboard.jwt_secret` from configuration
2. **Token Generation:** If a secret is configured, a JWT token is generated at startup
3. **Request Authentication:** All POST requests to `/v1/component_status` include the Bearer token
4. **Server Validation:** The status-dashboard validates the token signature using the same shared secret

## Token Validation

On the server side (status-dashboard), tokens should be validated by:

1. Extracting the token from the `Authorization` header
2. Verifying the HMAC-SHA256 signature using the shared secret
3. Optionally checking the claims (currently contains `{"stackmon": "dummy"}`)

## Security Considerations

### Secret Management

- **Never commit secrets** to version control
- Use environment variables (`MP_STATUS_DASHBOARD__JWT_SECRET`) in production
- Rotate secrets periodically
- Use strong, randomly-generated secrets (minimum 32 characters recommended)

### Transport Security

- Always use HTTPS for the status-dashboard URL in production
- The JWT token is sent in clear text in the Authorization header
- Without TLS, tokens could be intercepted and reused

### Token Characteristics

- **Stateless:** Tokens are self-contained and don't require server-side session storage
- **No Expiration:** Current implementation does not include expiration claims
- **Single Use Case:** Tokens are specifically for machine-to-machine authentication between reporter and status-dashboard

### Best Practices

1. **Use Strong Secrets:** Generate cryptographically secure random strings
   ```bash
   openssl rand -base64 32
   ```

2. **Environment Separation:** Use different secrets for development, staging, and production

3. **Audit Logging:** Log authentication failures on the status-dashboard for monitoring

4. **Network Isolation:** Where possible, restrict network access between components

## Example Implementation

### Generating a Token (Rust)

```rust
use hmac::{Hmac, Mac};
use jwt::SignWithKey;
use sha2::Sha256;
use std::collections::BTreeMap;

let secret = "your-shared-secret";
let key: Hmac<Sha256> = Hmac::new_from_slice(secret.as_bytes()).unwrap();

let mut claims = BTreeMap::new();
claims.insert("stackmon", "dummy");

let token_str = claims.sign_with_key(&key).unwrap();
let bearer = format!("Bearer {}", token_str);
```

### Validating a Token (Pseudocode)

```
function validate_token(authorization_header, secret):
    # Extract token from "Bearer <token>"
    token = extract_bearer_token(authorization_header)
    
    # Verify signature
    key = hmac_sha256_key(secret)
    claims = verify_and_decode(token, key)
    
    if claims is valid:
        return true
    else:
        return false
```
