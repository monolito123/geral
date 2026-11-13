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
| `slh-dsa-sha2-256s-keygen.txt` | All 10 SLH-DSA-SHA2-256s key-generation tests. The private key is stored as its SHA-256 digest. | NIST ACVP, `SLH-DSA-keyGen-FIPS205` |
| `slh-dsa-sha2-256s-siggen.txt` | Two deterministic signing tests (external interface, pure): a one-byte message with a 254-byte context, and an empty context. The expected signature is stored as its SHA-256 digest. | NIST ACVP, `SLH-DSA-sigGen-FIPS205` |
| `slh-dsa-sha2-256s-sigver.txt` | Eight verification tests (external interface, pure): two valid; four invalid by content (modified message, R, FORS and hypertree parts); two invalid by length (too small, too large), stored as the length only. | NIST ACVP, `SLH-DSA-sigVer-FIPS205` |

The NIST files were read from
<https://github.com/usnistgov/ACVP-Server> at commit
`975de31eb83d87039ec88934fdc47d8c312b892d` on 2026-11-13. SHA-256 of the
`internalProjection.json` files used:

```text
e67ee6540d40e11506c3c4e3b1f79fc1cefcd49820db99fc61f87cc8ba463baf  ML-DSA-keyGen-FIPS204
72dcaf5f69853ca267ccd16af9cb40949786aca0fcfbf05d1ebeba132b93af22  ML-DSA-sigGen-FIPS204
47cdd6314c7f746d02421ffcba89d4dbc7bb875ac49e07a029fdfc26fba55437  ML-DSA-sigVer-FIPS204
d7c53a1b6450087047b57aae83a5a51a0ac89ecdb23ebe071e83fbb69ae9d920  SLH-DSA-keyGen-FIPS205
62b42e7c27fda5de8a94aba2943ae413b59b6ea08141f3bc035b1eceae235dba  SLH-DSA-sigGen-FIPS205
a013fc2104f4ed4799d96d51141f65b965969b2cf10646626a021b6d456ce792  SLH-DSA-sigVer-FIPS205
```

NIST test vectors are works of the United States government and are in the
public domain. Ed25519 vectors from RFC 8032, section 7.1, are written out
in the test source.
