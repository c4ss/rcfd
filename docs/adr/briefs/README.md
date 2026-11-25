# Briefs

A brief is what the lead tells an agent when it issues a piece of work. Each one is kept here as a file, written and
committed by the lead on main before the agent starts ([lead process](../../process/lead-process.md)). It is the record that
the work was issued, of what the agent was told to read, and of what produced the result: the rules version and the model.
The registration cites its brief by path ([agent rules](../../process/agent-rules.md), rule 1).

The file is named after the branch, `<branch>.md`, with any `/` in the branch name written as `-`. One file per dispatched
branch, in both streams. It begins with these lines, which `scripts/process_report.py` reads by their prefix:

    Branch: <branch name>
    Items: <F-ids separated by commas, or "none">
    ADR: <the number the lead gave, or "none">
    Rules version: <version of docs/process/agent-rules.md>
    Model: <the model that does the work>
    Issued: <YYYY-MM-DD>
    Size (lead): <outcome>; <further registrations>; <heavy runs>; <questions for the maintainer>; <bytes to read>

Then two sections: `## Reading list`, one path per line with its size in bytes, and `## Task`.
From 2026-11-25 also `## Goal`: the one line starting `/goal` that states the end state of the work ([lead process](../../process/lead-process.md), "Issuing" 4).

A dispatch that is abandoned gets one appended line: `Closed without merge (<YYYY-MM-DD>): <why>`. A brief with no such line
whose branch no ledger line names is work that is issued and not merged.

These pages are not ADRs: they have no number and the test index does not read them.
