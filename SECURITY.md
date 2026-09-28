# Security policy

## Supported versions

Security fixes are made for the latest released version of Heine. Pre-release
builds may change before a fix is released, but reports against them are
welcome.

## Reporting a vulnerability

Do not report a suspected vulnerability in a public issue, pull request, or
discussion. Email [tweitzel@gmail.com](mailto:tweitzel@gmail.com) with:

- the Heine version and operating system;
- a minimal reproduction or proof of concept;
- the impact you expect; and
- any relevant configuration, paths, and diagnostic output.

Please give the maintainer a reasonable opportunity to investigate and fix the
problem before disclosing it publicly. The maintainer does not offer a response
time guarantee, but will acknowledge and assess reports when possible.

## Scope

Reports are in scope when they show that Heine can violate one of its checked
boundaries. Examples include bypassing the external-renderer permission,
escaping a checked source or output path, following a prohibited symbolic link,
or causing a renderer result to write outside the managed output contract.

An external renderer is a program selected and permitted by the site author.
Heine does not treat its temporary workspace as a security sandbox. A
vulnerability in a renderer program, or harmful behavior by a renderer that
the author chose to run, is outside this policy unless Heine itself bypasses a
documented boundary.
