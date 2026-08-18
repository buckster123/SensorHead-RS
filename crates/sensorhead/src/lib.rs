//! SensorHead-RS core library.
//!
//! Types, parsers, and pure views over the `:8080` contract. Hardware I/O and the
//! two vendor walls (BSEC2, libcamera/Picamera2) stay out of this crate until a
//! later slice names the FFI shape.
//!
//! See `docs/design.md` for the contract and `docs/CHARTER.md` for the binding
//! decisions. The Python original is the read-only checkout at `py-source/`.
