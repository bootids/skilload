# Unicode 15.1 Normalization and Case-Folding Reference

Scope: Unicode Standard and Unicode Character Database (UCD) version 15.1.0 rules used by skilload Revision 1 Library tags and portable Skill-tree collision keys.

Last updated: 2026-08-18.

## Why It Matters

`SKL-LIB-008` needs portable tag whitespace trimming, canonical composition, and locale-independent caseless comparison. `SKL-SRC-008` needs a host-independent first-pass key for canonically equivalent or case-only Skill-tree paths. Implementations must use the same versioned Unicode inputs or imports, hashes, and collision decisions could disagree.

## Key Conclusions

* Unicode 15.1.0 is an archived, immutable UCD release. Use its versioned files rather than the moving `UCD/latest` aliases.
* The `White_Space` property is defined by the versioned `PropList.txt`. Revision 1 trims exactly that property from both ends before validation; it does not infer whitespace from a runtime library's current Unicode version.
* Normalization Form C (NFC) performs canonical decomposition followed by canonical composition. It makes canonically equivalent composed and decomposed input share one display spelling without applying compatibility equivalence.
* Unicode full default case folding uses `CaseFolding.txt` status `C` and `F` mappings. Status `S` is the simple alternative; status `T` is Turkic tailoring and is excluded from the default operation.
* Case folding does not preserve normalization. Revision 1 therefore normalizes the display spelling to NFC, applies the full `C` plus `F` mapping, and normalizes the comparison result to NFC again.
* Unlisted case-folding code points map to themselves. The case-folded value is an internal comparison/index key, not the user-visible tag spelling.
* Revision 1 path collision keys require valid UTF-8 and apply NFD, full default case folding (`C` plus `F`, excluding `T`), then NFD again to each path segment before joining segments with literal `/`. NFD is used here so canonically equivalent composed/decomposed paths share a key; the original normalized Git path bytes remain the display, materialization, and integrity bytes.
* Unicode equivalence cannot model every host filesystem rule. After the portable key check, materialization still needs an exclusive target-filesystem alias probe. A host-only collision rejects that materialization without changing the portable lock or digest.

## Cautions

Do not substitute locale-sensitive lowercasing, simple case folding, NFKC case folding, or a newer runtime's unpinned tables. A future Unicode-data or algorithm change requires the explicit metadata/schema or integrity-format migration required by the owning behavior; it must not silently rewrite existing keys or reinterpret a lock.

## Sources

* [Unicode 15.1.0 components](https://www.unicode.org/versions/components-15.1.0.html)
* [Unicode 15.1.0 CaseFolding.txt](https://www.unicode.org/Public/15.1.0/ucd/CaseFolding.txt)
* [Unicode 15.1.0 PropList.txt](https://www.unicode.org/Public/15.1.0/ucd/PropList.txt)
* [Unicode Standard Annex #15: Normalization Forms, Revision 53](https://www.unicode.org/reports/tr15/tr15-53.html)
* [Unicode Standard Annex #44: Unicode Character Database, Revision 31](https://www.unicode.org/reports/tr44/tr44-31.html)
