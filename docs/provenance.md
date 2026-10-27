# Provenance record

This page says where rcfd's knowledge comes from and lists, in one place, every recorded case in which vendor
documentation was consulted while rcfd was built. It is for users and contributors who need to judge the clean-room claim
for themselves. The rules for new contributions are in `CONTRIBUTING.md` at the repository root.

## The rule

Algorithms, model constants, file formats and benchmark data are to come from published literature (papers, textbooks,
public reports, public format specifications) and from first principles. No vendor source code, binaries or install contents
were used, and no vendor documentation text was copied into the code. A few model constants in the code were found only in
vendor documentation; item 3 lists them, and they are marked UNVERIFIED. No decompilation, disassembly or other inspection of vendor
binaries was ever done; an audit of the full history on 2026-10-27 found no decompiler output and no reverse-engineering
artefact in any commit.

The cases below are all the exceptions on record. They come from the project's own notes, ADRs and audits. Each per-wave
"clean-room" sentence in an ADR describes only that wave's own work.

## What was consulted, and for what

1. **Vendor product pages (public), 2026-10-17.** Read to list capabilities for the [parity matrix](parity-matrix.md)
   (S1, S2) and the [product suites backlog](product-suites.md) (P1 to P20). Capability names were paraphrased; no text was
   copied.
2. **Vendor online help (access normally needs a login), 2026-10-17.** Mostly contents pages: the contents and chapter
   introductions of two CFD theory guides, a CFD user's guide and a CFD modelling guide, a GPU solver supported-features
   list, one transient blade-row guidelines page, and the section headings of a structural theory reference (parity matrix
   S3 to S10, product suites P21). Pages were read through a web-fetch tool that returns a condensed extraction. They were
   used only to name capabilities and group them; the descriptions, priorities and judgements in those two pages are rcfd's
   own. Whether any short phrase of the vendor pages survives in the paraphrases is UNVERIFIED (checking would need the
   vendor pages again). Search-result snippets of vendor pages and webinars were also seen and are marked as snippets there.
3. **Vendor documentation as a secondary check of model constants, 2026-10-13 to 2026-10-18.** Some constants of
   published models were checked against, or found only in, vendor theory pages: pages of the vendor's online help (login
   required) and an unofficial web copy of an older vendor theory guide. These checks are no longer counted as sources. The
   affected values keep their literature attribution and are marked UNVERIFIED in [reference data](reference-data.md) and
   the [verification report](verification-report.md) until a published source confirms them:
   * Realizable k-epsilon `A0 = 4.04` (the published report prints 4.0; 4.04 stays the default, ADR 0017);
   * the log-law pair `kappa = 0.4187`, `E = 9.793` (rounded to 0.41 and 9.8 in the code);
   * the two-layer constants `A_mu = 70`, `A_eps = 2 c_l`, `Re_y* = 200` and the tanh blend form (attributed to Chen and
     Patel 1988 and Jongen 1998);
   * the Zeldovich NO rate constants (attributed to Hanson and Salimian; none is bundled, the tests use invented values);
   * the Proudman constant `alpha = 0.1` with `M_t = sqrt(2k)/c0` (attributed to Proudman 1952 and Sarkar and Hussaini).
   The RNG and realizable k-epsilon constants of the first commits were first checked the same way and were later
   re-sourced to papers. The Kader blend constants, the Cebeci-Bradshaw roughness regimes and the scalable wall function
   concept were also seen there but are confirmed by, or attributed to, other sources. An earlier version of the
   verification report quoted one short phrase (about 45 characters) of a vendor guide, with attribution; it was removed.
4. **One experiment, ADR 0063 (2026-10-18).** The equilibrium standard wall-function forms (the classical Launder and
   Spalding 1974 forms; the paper was not read) were taken from the unofficial web copy named in item 3 and implemented as
   an opt-in treatment. It missed its registered limits, and its code was removed in the same wave; no code from it
   remains. The forms are labelled UNVERIFIED in [ADR 0063](adr/0063-wave31b-turbulence-wall-diagnosis.md).
5. **Early roadmap wording, 2026-10-13 to 2026-10-22.** The first roadmap's gap notes for about 20 items named settings and
   modules of commercial products, and the roadmap cited an inventory kept outside the repository. According to the
   project's own audit of 2026-10-14, those names came from file listings of a commercial product installed on the author's
   machine. Only names were used: no code, text or data. The notes were reworded with generic capability names
   (2026-10-13 to 2026-10-22), and the inventory is not part of the repository.
6. **Vendor material on the author's machine.** The author's machine also holds a commercial product and other vendor
   material. The project rule forbids using it for rcfd. Apart from the file names in item 5, the records state that
   nothing from it was read.

## File formats

* Fluent `.msh` / `.cas` reader (2026-10-13): written from memory of a public description of the section layout; that
  description is not named in the record, so its source is UNVERIFIED. The reader was checked only as a black box against a
  public sample file (`elbow.msh` from an OpenFOAM tutorial, GPL, fetched and never committed). It predates the help pages
  of item 2. See [mesh import](mesh-import.md).
* EnSight Gold export: written from the vendor's public description of the file format (ADR 0048).
* Nastran-style bulk data export: from memory of the public Nastran manuals (ADR 0051).

## Third-party data and code

Data tables and their licences are listed in `NOTICE` and `validation-data/manifest.json` (see
[validation data](validation-data.md)).
