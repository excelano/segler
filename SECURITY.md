# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately, using the **Report a vulnerability** button on the Security tab of the repository concerned. If that button is not there, or you would rather not go through GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest release receives security fixes. Older versions are not supported. A fix ships in a new release; update the way you installed.

## What Segler can access

Segler runs locally on your machine. It reads the DocLang file or archive you open, holds it in memory while you work, and writes a file back only when you save. A save writes a temporary file beside the target and renames it over the target. A `.dclx` archive is a ZIP; Segler reads only the parts the DocLang archive format names and never executes anything it finds inside one. It makes no network calls, has no auth layer, and can only read and write files your operating-system user already has access to.

The `segler corpus --toolkit` command runs the reference `doclang` program from your `PATH` on each file it checks. No other command starts another program.

## What Segler stores

Segler stores nothing outside the files you explicitly save. There is no config directory, no telemetry, no analytics, and no remote logging.
