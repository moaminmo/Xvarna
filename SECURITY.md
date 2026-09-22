# Security Policy

## Supported versions

Before the public 1.0 release, only the latest commit on `main` is supported.
After 1.0, the latest minor release and current development branch will receive
security fixes unless a release notice states otherwise.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Until a dedicated
security address is published, use GitHub private vulnerability reporting on
the repository. Include:

- affected commit or version;
- operating system and architecture;
- a minimal reproduction;
- expected and observed behavior;
- impact assessment;
- whether untrusted files or network input are involved.

The project will acknowledge a valid private report, investigate it, and
coordinate disclosure. No fixed response-time SLA is promised before a funded
maintainer team exists.

## Scope

Security-sensitive surfaces include file parsers, native FFI, cache readers,
external process execution, report archives, package loading, and GPU buffers.
XVARNA does not upload project geometry by default.

