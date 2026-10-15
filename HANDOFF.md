# Handoff: donguru-api (planning session, P2534#15oct-2031)

Done (15 Oct 2026):
- Design in tpm: `~/git/tpm/prj/P2534-donguru/design/api-v1.md` (API tree, CLI tree, versioning, budget,
  wire_weaver gaps, pieces, open decisions D1-D4).
- Pieces filed: P2534#15oct-2043 .. #15oct-2044l (API-8..API-18, CORE-1/3, CLI pieces), ids bound to the tasks.
- wire_weaver proposals: P2401#15oct-2045, -2045b, -2045c.
- FEATURES.md: planned items API-8..API-18, CLI-19..CLI-24 (this branch).

Next:
- Wait for Roman's D1-D4 (the skeleton API-8 depends on D1/D2), then spawn API-8 on omarchy-m1, then the waves of the
  design's Pieces section.

Gotchas:
- This session holds wire_weaver SRV-18 / CLT-28 only because the task text names them; multi-observe owns them.
- One B153A on omarchy-m1: at most two firmware sessions flash there at a time.
- Baseline size (release, 15 Oct): FLASH 115.6 KiB, RAM 10.9 KiB; app budget 200 KiB for A/B DFU later.
