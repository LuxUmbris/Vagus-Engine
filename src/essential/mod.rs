//! Portable public audio output API.
//!
//! Writes use interleaved floating-point samples in the range `-1.0..=1.0`. The return
//! value is accepted frames, not samples; short writes and zero indicate backpressure.
//! Retry the unaccepted suffix. `drain` is an explicit completion barrier and may wait.

//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS OR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

pub use vagus_essential::*;
