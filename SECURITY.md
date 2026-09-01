# Security Policy

## Supported versions

Security fixes are provided for the latest published minor release. Older
minor releases may be asked to upgrade before receiving a fix.

| Version | Supported |
| --- | --- |
| Latest published minor | Yes |
| Older minors | No |

## Reporting a vulnerability

Do not disclose a suspected vulnerability in a public GitHub issue, discussion,
or pull request.

Use GitHub's private vulnerability reporting page:

<https://github.com/z0horizon/ip-discovery/security/advisories/new>

If private reporting is unavailable, open a public issue requesting a private
contact channel without including exploit details or sensitive information.

Include, when possible:

- the affected package and version;
- the operating system and enabled Cargo features;
- a minimal reproduction or malformed packet sample;
- the expected and observed behavior;
- the security impact and any known mitigations.

The maintainers will acknowledge reports as soon as practical, validate the
impact, coordinate a fix and release, and credit the reporter unless anonymity
is requested. Please allow time for a patched release before public disclosure.

Relevant security areas include DNS/STUN response validation, parser safety,
timeout or resource-exhaustion behavior, dependency vulnerabilities, CLI
artifacts, and Node.js native bindings.

## Scope

Public IP providers are external services and their availability is not a
security guarantee. Reports about a provider being temporarily unavailable or
returning a different public IP are normally operational issues unless they
demonstrate spoofing, unsafe parsing, credential exposure, or another concrete
security impact.
