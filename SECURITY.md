# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately through GitHub Security Advisories at https://github.com/excelano/segler/security/advisories/new. If you would rather not use GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest 0.x release receives security fixes. Older versions are not supported.

## What Segler can access

Segler runs locally on your machine. It reads the DocLang file or archive you open, holds it in memory while you work, and writes a file back only when you save. A `.dclx` archive is a ZIP; Segler reads only the parts the DocLang archive format names and never executes anything it finds inside one. It makes no network calls, has no auth layer, and can only read and write files your operating-system user already has access to.

## What Segler stores

Segler stores nothing outside the files you explicitly save. There is no config directory, no telemetry, no analytics, and no remote logging.

## Verifying releases

Every GitHub release includes a `.sha256` file next to each archive listing its SHA-256 hash. Verify any download before running it. Release artifacts are built by GitHub Actions from a tagged commit; the workflows in this repository are public and auditable.
