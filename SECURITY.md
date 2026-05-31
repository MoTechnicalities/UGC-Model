# Security Policy

## Supported Scope

Security reports are welcomed for:

- API server behavior in `src/main.rs`
- Input validation and parser safety
- Deterministic replay and audit integrity risks
- Data exposure or unsafe defaults
- Dependency vulnerabilities in the Rust supply chain

## Reporting a Vulnerability

Please report vulnerabilities privately and do not open a public issue first.

Include:

- Affected component and file path
- Reproduction steps
- Impact assessment
- Suggested mitigation if available

Use the maintainer contact listed in `README.md` for initial private reporting.

## Response Expectations

- Initial acknowledgement target: within 7 days
- Triage and severity assessment: as quickly as practical
- Fix timeline: based on severity and exploitability

## Disclosure Policy

- Coordinate disclosure with maintainers
- Avoid publishing exploit details before a fix or mitigation is available
- Credit reporters in release/changelog notes when appropriate
