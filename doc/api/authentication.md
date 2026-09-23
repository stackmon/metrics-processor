# Status Dashboard Authentication

## Overview

The `cloudmon-metrics-reporter` authenticates against the Status Dashboard with a Zitadel OIDC
service identity. MP does not implement the OAuth flow itself: the JWT Profile exchange is delegated
to the community [`zitadel`](https://crates.io/crates/zitadel) crate (`zitadel::credentials`).

The reporter loads the Zitadel machine user key file once at startup. For every report the crate

1. discovers the token endpoint from `{issuer}/.well-known/openid-configuration`,
2. signs a short-lived JWT Profile assertion (RS256) with the private key of the key file,
3. exchanges it at the discovered token endpoint with the
   `urn:ietf:params:oauth:grant-type:jwt-bearer` grant,

and MP only wraps the returned access token into an `Authorization: Bearer <access_token>` header.
No client secret is involved anywhere, and the Status Dashboard verifies the issued token against
the Zitadel issuer.

| Component                | Responsibility                                                    |
|--------------------------|-------------------------------------------------------------------|
| MP (`src/oidc.rs`)       | Load the key file once, call the crate per request, build headers |
| `zitadel` crate          | OIDC discovery, JWKS fetch, assertion signing, token request      |
| Status Dashboard backend | Token verification (`SD_OIDC_*` settings)                         |

## Machine User Key File

The reporter authenticates as a Zitadel **machine user** (service user). The key file is downloaded
from the Zitadel Console (instance → *Users* → *Service Users* → select the machine user → *Keys* →
*New* → download the JSON key file):

```json
{
  "type": "serviceaccount",
  "keyId": "392067695547252958",
  "key": "-----BEGIN RSA PRIVATE KEY-----\n...\n-----END RSA PRIVATE KEY-----",
  "userId": "392040635458125910"
}
```

| Field    | Meaning                                                                                              |
|----------|--------------------------------------------------------------------------------------------------------|
| `type`   | Must be `serviceaccount`; any other value aborts the reporter at startup with the configuration key named |
| `keyId`  | Key id sent as the `kid` header of the assertion, used by Zitadel to select the public key              |
| `key`    | PEM encoded RSA private key; Zitadel emits PKCS#1 (`BEGIN RSA PRIVATE KEY`), PKCS#8 is accepted too     |
| `userId` | Machine user id, used as `iss` and `sub` of the assertion                                               |

Notes:

- The path is configured through `status_dashboard.oidc_key_file`
  (`MP_STATUS_DASHBOARD__OIDC_KEY_FILE`). The file is read and validated exactly once at startup, so
  it does not have to stay available after the reporter started.
- Zitadel *application* keys (`type: application`, `clientId`) are not accepted: the `zitadel` crate
  implements the machine user JWT Profile flow, which is the key kind this deployment uses. Such a
  key file aborts startup with an error naming `MP_STATUS_DASHBOARD__OIDC_KEY_FILE`.
- Keep the key file out of the repository and out of configuration files, for example by mounting it
  as a secret volume or by writing it to a container file system path.
- The key material, the signed assertion and the access token are never logged, and the private key
  never appears in an error message. `OidcIdentity` redacts the loaded credentials in its `Debug`
  output.

## Flow

1. The reporter resolves its service identity from the configuration
   (`status_dashboard.oidc_issuer`, `oidc_key_file`, `oidc_scopes`). A missing or unusable value
   aborts the reporter at startup and the error names the offending configuration key.
2. Before every report the crate signs a new assertion and exchanges it for an access token.
   Tokens are never cached in-process, so every report carries a fresh token.
3. The access token is sent as `Authorization: Bearer <access_token>` on every Status Dashboard
   call.
4. The Status Dashboard validates the token with `go-oidc`: the issuer must match `SD_OIDC_ISSUER`,
   the audience must contain `SD_OIDC_CLIENT_ID`, and the signing key is resolved from the JWKS
   endpoint by `kid`.
5. The reporter role is read from the project roles claim
   (`urn:zitadel:iam:org:project:roles`, configurable on the backend via `SD_OIDC_ROLES_CLAIM`) and
   mapped to the reporter role configured by `SD_RBAC_ROLES_REPORTERS`.

## Assertion

The assertion signed by the crate for the machine user looks like this:

```json
{
  "alg": "RS256",
  "kid": "392067695547252958",
  "typ": "JWT"
}
```

```json
{
  "iss": "392040635458125910",
  "sub": "392040635458125910",
  "aud": "https://zitadel.example.com",
  "iat": 1705929045,
  "exp": 1705932645
}
```

`iss` and `sub` are the `userId` of the machine user, the audience is the issuer URL and `exp` is one
hour after `iat`.

## Token Request

The reporter presents the assertion as the `assertion` parameter of the JWT bearer grant, without
any `Authorization` header and without a client id:

```bash
curl -sS -X POST "https://zitadel.example.com/oauth/v2/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer" \
  -d "assertion=<signed assertion>" \
  -d "scope=<space joined scopes>"
```

Notes:

- The token endpoint is discovered per request from `{issuer}/.well-known/openid-configuration`, so
  only the issuer is configured.
- The `zitadel` crate always adds the `openid` scope in front of the configured scopes and sends the
  whole list space-joined as a single `scope` parameter, in the configured order.

### Required scopes

The configured scope list has to contain both of these scopes:

| Scope                                                      | Why it is needed                                                                    |
|------------------------------------------------------------|-------------------------------------------------------------------------------------|
| `urn:zitadel:iam:org:project:role:sd_reporters`             | Puts the reporter project role into the token, so the status page RBAC check passes |
| `urn:zitadel:iam:org:project:id:<projectId>:aud`            | Makes the token audience the project id instead of the client id                    |

`<projectId>` is the Zitadel project that both the Status Dashboard and the machine user are part
of. Zitadel puts the client id into the token audience by default, and the Status Dashboard rejects
that with `expected audience ... got [...]`; requesting the audience scope switches the audience to
the project id, so the backend has to be configured with `SD_OIDC_CLIENT_ID=<projectId>`.

```yaml
status_dashboard:
  oidc_scopes:
    - "urn:zitadel:iam:org:project:role:sd_reporters"
    - "urn:zitadel:iam:org:project:id:392066917738875090:aud"
```

`oidc_scopes` has no default, because the audience scope contains the project id of the deployment.
The reporter fails at startup when the list misses either scope and names
`MP_STATUS_DASHBOARD__OIDC_SCOPES` in the error.

Verified against the pre-production instance with those two scopes: the access token carried
`aud = ["390700708019568682"]` and `groups = ["sd_reporters"]`, with `iss` set to the issuer URL.

## Report Authentication

```http
POST /v2/events HTTP/1.1
Host: status.example.com
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "title": "System incident from monitoring system",
  "description": "Object Storage Service is degraded",
  "impact": 2,
  "components": [218],
  "start_date": "2024-01-22T10:30:44Z",
  "system": true,
  "type": "incident"
}
```

## Permission Boundary

The Zitadel service identity used by the reporter is restricted to a single operation:

- Allowed: `POST /v2/events` with `"system": true`
- Denied: component read endpoints and any event without `system: true`

The Status Dashboard backend enforces this boundary and rejects reporter-scoped tokens on every
other route, independently of the roles present in the token.

## Backend Verification Points

| Check         | Backend setting          | Expectation                                                |
|---------------|--------------------------|------------------------------------------------------------|
| Issuer        | `SD_OIDC_ISSUER`         | Matches `status_dashboard.oidc_issuer`                     |
| Audience      | `SD_OIDC_CLIENT_ID`      | The project id, requested through the audience scope        |
| Signing key   | JWKS by `kid`            | Resolved from the issuer                                   |
| Roles claim   | `SD_OIDC_ROLES_CLAIM`    | `urn:zitadel:iam:org:project:roles`                        |
| Reporter role | `SD_RBAC_ROLES_REPORTERS` | Role key requested through `oidc_scopes`                  |

The reporter does not send an audience of its own: Zitadel decides the audience of the access token,
so the backend has to be configured with the audience the requested scopes produce:

- Without the audience scope the token audience is the client id of the machine user, which the
  backend rejects with `expected audience ... got [...]`.
- With `urn:zitadel:iam:org:project:id:<projectId>:aud` the audience is the project id, which is
  what `SD_OIDC_CLIENT_ID` has to be set to.

## Failure Handling

The reporter is fail-closed:

- A missing `oidc_issuer`, `oidc_key_file` or `oidc_scopes`, an unreadable or invalid key file, a key
  type other than `serviceaccount`, an empty `keyId`/`key`/`userId` and a scope list without a role
  scope or without the project audience scope abort the reporter at startup, and the error names the
  configuration key that has to be fixed.
- A failing discovery or token request, and a token response without a usable `access_token`, abort
  the report instead of sending it without authentication.
- The private key, the signed assertion and the access token are never part of an error message or
  of the reporter output.

## Configuration

```yaml
status_dashboard:
  url: "https://status.example.com"
  oidc_issuer: "https://zitadel.example.com"
  oidc_key_file: "/etc/cloudmon/service-identity.json"
  oidc_scopes:
    # Reports the reporter role and makes the token audience the project id
    - "urn:zitadel:iam:org:project:role:sd_reporters"
    - "urn:zitadel:iam:org:project:id:392066917738875090:aud"
```

Environment variable equivalents:

| Environment Variable                 | Configuration path               |
|--------------------------------------|----------------------------------|
| `MP_STATUS_DASHBOARD__URL`           | `status_dashboard.url`           |
| `MP_STATUS_DASHBOARD__OIDC_ISSUER`   | `status_dashboard.oidc_issuer`   |
| `MP_STATUS_DASHBOARD__OIDC_KEY_FILE` | `status_dashboard.oidc_key_file` |
| `MP_STATUS_DASHBOARD__OIDC_SCOPES`   | `status_dashboard.oidc_scopes`   |
