# VRAND

**VRAND — Verifiable Randomness from Live Public Data**

VRAND is an open-source experimental randomness system designed to derive reproducible randomness from continuously changing public data streams.

The first version of VRAND uses two live public sources:

* Wikimedia EventStreams — RecentChange
* RIPE RIS Live — routing information

The goal is to provide a simple and transparent mechanism where the derivation process can be independently understood, reproduced, and verified.

---

## How VRAND Works

VRAND continuously receives data from the configured public streams.

For each source, VRAND maintains a rolling buffer containing the latest:

**262,144 bytes (256 KiB)**

Each source buffer is independently hashed using SHA-256.

The two resulting hashes are then concatenated and hashed again to produce the final VRAND output.

### Wikimedia

```text
Wikimedia live stream
        ↓
262,144-byte rolling buffer
        ↓
SHA-256
        ↓
H1
```

### RIPE RIS

```text
RIPE RIS live stream
        ↓
262,144-byte rolling buffer
        ↓
SHA-256
        ↓
H2
```

### Final VRAND hash

The two 32-byte SHA-256 digests are concatenated:

```text
H1 || H2
```

The final VRAND hash is then:

```text
H3 = SHA-256(H1 || H2)
```

Where:

* `H1` = SHA-256 of the current Wikimedia buffer
* `H2` = SHA-256 of the current RIPE RIS buffer
* `H3` = final VRAND output
* `||` = byte concatenation

---

## Data Sources

### Wikimedia EventStreams

VRAND currently uses the Wikimedia RecentChange stream:

```text
https://stream.wikimedia.org/v2/stream/recentchange
```

This is a live EventStreams feed containing recent Wikimedia changes.

### RIPE RIS Live

VRAND currently uses:

```text
https://ris-live.ripe.net/v1/stream/?format=json
```

This provides a live stream of RIPE RIS routing information.

---

## Data Processing

VRAND intentionally keeps the current derivation mechanism simple.

The source payload bytes are fed directly into the rolling buffer.

The current implementation does not perform additional transformations such as:

* JSON field reordering
* JSON sorting
* statistical entropy extraction
* multiple hashing rounds
* custom randomness transformations

The current source-processing model is:

```text
Public source
     ↓
UTF-8 payload bytes
     ↓
Rolling 262,144-byte buffer
     ↓
SHA-256
```

The current protocol therefore aims to minimize unnecessary processing between the public source and the resulting hash.

---

## Rolling Buffers

Each source has its own independent rolling buffer.

The buffer always contains the most recent 262,144 bytes received from that source.

Conceptually:

```text
Older data
    ↓
[------------------------------------------]
                    262,144 bytes
                         ↑
                    newest data
```

When new bytes arrive, the oldest bytes are removed so that the buffer remains at the configured size.

This allows VRAND to operate continuously without storing the entire history of the live streams.

---

## Current CLI

VRAND currently provides three main commands.

### `live`

Start the live VRAND dashboard:

```bash
vrand live
```

The dashboard displays:

* current UTC time
* Wikimedia connection status
* RIPE RIS connection status
* received messages
* received bytes
* rolling buffer size
* Wikimedia SHA-256
* RIPE SHA-256
* current final VRAND hash

Example:

```text
==============================================================
                     VRAND LIVE 0.1
==============================================================

UTC TIME          : 2026-09-07T19:50:32+00:00

WIKIMEDIA
  Status           : ● CONNECTED
  Messages         : 341
  Bytes received   : 173822
  Buffer           : 173822 / 262144 bytes
  SHA-256          : ...

RIPE RIS
  Status           : ● CONNECTED
  Messages         : 928
  Bytes received   : 245019
  Buffer           : 245019 / 262144 bytes
  SHA-256          : ...

--------------------------------------------------------------
                         DERIVATION
--------------------------------------------------------------

H1  WIKIMEDIA     : ...
H2  RIPE RIS      : ...

H3 = SHA256(H1 || H2)

VRAND OUTPUT      : ...
```

---

### `create-hash`

Create one VRAND hash from fresh live source data:

```bash
vrand create-hash
```

The command connects to both sources and waits until both rolling buffers contain 262,144 bytes.

It then produces:

* protocol version
* exact UTC creation time
* Wikimedia buffer size
* Wikimedia SHA-256
* RIPE buffer size
* RIPE SHA-256
* final VRAND hash

The derivation is:

```text
H1 = SHA-256(Wikimedia buffer)

H2 = SHA-256(RIPE buffer)

H3 = SHA-256(H1 || H2)
```

The creation timestamp is recorded as metadata and is not currently included in the hash calculation.

---

### `random-int`

Generate a random integer using the current VRAND output:

```bash
vrand random-int --min 1 --max 100
```

Example for a six-sided die:

```bash
vrand random-int --min 1 --max 6
```

The range is inclusive.

VRAND uses rejection sampling when mapping the hash output to a range in order to avoid modulo bias.

---

## Standalone Release Binaries

VRAND is intended to be usable without requiring users to install Rust, Cargo, or a development environment.

The project therefore uses GitHub Actions to build standalone native executables for supported operating systems and architectures.

Planned release targets include:

```text
Linux   x86_64
Linux   ARM64

Windows x86_64
Windows ARM64

macOS   x86_64
macOS   ARM64
```

Users will be able to download the executable matching their platform from GitHub Releases.

A release binary is intended to work as a portable CLI application without requiring the Rust toolchain.

---

## Building From Source

### Requirements

* Rust stable toolchain
* Cargo

Install Rust from:

```text
https://www.rust-lang.org/tools/install
```

Clone the repository:

```bash
git clone https://github.com/xxgrandmaster/vrand.git
cd vrand
```

Build:

```bash
cargo build --release
```

Run the live dashboard:

```bash
cargo run --release -- live
```

Create a hash:

```bash
cargo run --release -- create-hash
```

Generate a random integer:

```bash
cargo run --release -- random-int --min 1 --max 100
```

The compiled executable is located at:

```text
target/release/vrand
```

On Windows:

```text
target\release\vrand.exe
```

---

## Project Structure

```text
VRAND/
│
├── .github/
│   └── workflows/
│       └── release.yml
│
├── src/
│   ├── main.rs
│   └── buffer.rs
│
├── Cargo.toml
├── Cargo.lock
└── README.md
```

---

## Design Philosophy

VRAND is designed around a few simple principles.

### Transparency

The derivation process should be easy to inspect.

### Reproducibility

An independent implementation should be able to reproduce the same calculations when the required source data is available.

### Minimal transformation

The system should avoid unnecessary manipulation of the original public data before hashing it.

### Modularity

The randomness engine should eventually be usable as:

* a command-line tool
* a Rust library
* an API
* a component inside another application
* a source of randomness for games and other software

---

## Example Architecture

```text
                    PUBLIC DATA SOURCES

             ┌──────────────────────────┐
             │      Wikimedia            │
             │      EventStreams         │
             └────────────┬─────────────┘
                          │
                          ↓
                 Rolling 256 KiB Buffer
                          │
                          ↓
                       SHA-256
                          │
                          ↓
                         H1


             ┌──────────────────────────┐
             │       RIPE RIS            │
             │       Live Stream         │
             └────────────┬─────────────┘
                          │
                          ↓
                 Rolling 256 KiB Buffer
                          │
                          ↓
                       SHA-256
                          │
                          ↓
                         H2


                    H1 || H2
                        │
                        ↓
                     SHA-256
                        │
                        ↓
                   VRAND OUTPUT
```

---

## Project Roadmap

### v0.1.0 — Experimental Prototype

* [x] Wikimedia live source
* [x] RIPE RIS live source
* [x] Rolling source buffers
* [x] SHA-256 source hashes
* [x] Final VRAND hash
* [x] Live CLI dashboard
* [x] VRAND random integer generation
* [x] UTC timestamps for generated hashes
* [x] Basic cross-platform release workflow

### v0.2.0 — Verification

Planned:

* [ ] Formal VRAND event format
* [ ] Proof files
* [ ] `vrand verify`
* [ ] Source event identifiers
* [ ] Better reproducibility
* [ ] Formal protocol specification
* [ ] Automated tests
* [ ] Cross-platform verification testing

### v0.3.0 — Library and Integration

Planned:

* [ ] Rust library API
* [ ] Stable programmatic interface
* [ ] JSON output mode
* [ ] Additional randomness tools
* [ ] API/server mode
* [ ] Documentation for developers integrating VRAND

### Future

Possible future work includes:

* additional public data sources
* web dashboard
* graphical interface
* public VRAND API
* game integrations
* lotteries and provably fair applications
* independent implementations in other programming languages

---

## Security and Randomness Disclaimer

VRAND 0.1 is an experimental research and software project.

The fact that a value is derived from public data and cryptographically hashed does not automatically make that value suitable for every security-sensitive application.

The security properties of VRAND depend on factors including:

* unpredictability of the source data
* influenceability of the source data
* availability of historical source data
* timing and event selection
* possible manipulation or withholding of source data
* the exact application consuming the result

VRAND 0.1 should therefore **not** be considered a replacement for a cryptographically secure operating-system random number generator for purposes such as:

* cryptographic key generation
* passwords
* authentication secrets
* private keys
* security tokens
* other security-critical secrets

Likewise, VRAND 0.1 should not yet be considered a proven randomness oracle for gambling, financial systems, or other high-stakes applications.

The project's future protocol and verification work is intended to formally define these properties.

---

## Why VRAND Exists

Traditional randomness systems can be secure while remaining opaque to ordinary users.

VRAND explores a different idea:

> Can continuously changing public information be transformed into randomness using a simple, transparent, and independently reproducible process?

The project is an attempt to build that system openly.

---

## Contributing

Contributions, experiments, independent implementations, security reviews, protocol critiques, and reproducibility testing are welcome.

Particularly useful contributions include:

* identifying weaknesses in the protocol
* testing source reliability
* building independent verifiers
* testing cross-platform behavior
* analyzing the randomness properties
* proposing improvements to the verification model

---

## License

MIT License

Copyright (c) 2026

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files, to deal in the Software
without restriction, including without limitation the rights to use, copy,
modify, merge, publish, distribute, sublicense, and/or sell copies of the
Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
