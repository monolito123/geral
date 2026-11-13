# Test vectors

Known-answer tests for the algorithms `nott-crypto` builds on, taken from
their standards. They check that NottOS calls the implementations with the
right parameter set and interface; they are not a substitute for the
implementations' own test suites.

| File | Content | Source |
|---|---|---|
| `ml-dsa-87-keygen.txt` | All 25 ML-DSA-87 key-generation tests. Keys are stored as SHA-256 digests to keep the file small. | NIST ACVP, `ML-DSA-keyGen-FIPS204` |
| `ml-dsa-87-siggen.txt` | Two deterministic signing tests (external interface, pure), one with an empty context. The expected signature is stored as its SHA-256 digest. | NIST ACVP, `ML-DSA-sigGen-FIPS204` |
| `ml-dsa-87-sigver.txt` | Six verification tests (external interface, pure): two valid, one with a 255-byte context; four invalid (modified message, commitment, hint and response). | NIST ACVP, `ML-DSA-sigVer-FIPS204` |

The NIST files were read from
<https://github.com/usnistgov/ACVP-Server> at commit
`975de31eb83d87039ec88934fdc47d8c312b892d` on 2026-11-13. SHA-256 of the
`internalProjection.json` files used:

```text
e67ee6540d40e11506c3c4e3b1f79fc1cefcd49820db99fc61f87cc8ba463baf  ML-DSA-keyGen-FIPS204
72dcaf5f69853ca267ccd16af9cb40949786aca0fcfbf05d1ebeba132b93af22  ML-DSA-sigGen-FIPS204
47cdd6314c7f746d02421ffcba89d4dbc7bb875ac49e07a029fdfc26fba55437  ML-DSA-sigVer-FIPS204
```

NIST test vectors are works of the United States government and are in the
public domain. Ed25519 vectors from RFC 8032, section 7.1, are written out
in the test source.
