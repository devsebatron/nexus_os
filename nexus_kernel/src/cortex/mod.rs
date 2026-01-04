use alloc::format;
use alloc::string::String;
use x86_64::instructions::interrupts;

pub struct CortexEngine {
    _private: (),
}

// Simulated 1.58-bit weights: -1, 0, 1 stored as i8
static WEIGHTS: [i8; 4] = [1, -1, 0, 1];

impl CortexEngine {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn infer(&self, input: &[f32]) -> String {
        // CRITICAL: We must disable interrupts to prevent context switches
        // while usage AVX/YMM registers, as the scheduler does not save them.
        interrupts::without_interrupts(|| {
            // SAFETY: We are in a critical section (no interrupts), so it is safe
            // to use AVX instructions locally without corrupting other tasks.
            let result = unsafe { compute_avx(input, &WEIGHTS) };
            format!("Cortex [SafeMode]: BitNet Activation = {:.4}", result)
        })
    }
}

// Enable AVX locally for this function only.
// This prevents the compiler from generating AVX code elsewhere where it might represent a risk.
#[target_feature(enable = "avx")]
unsafe fn compute_avx(input: &[f32], weights: &[i8]) -> f32 {
    let mut dot_product = 0.0;

    // In a real implementation, we would use _mm256_load_ps etc.
    // For this simulation, the target feature ensures we CAN use vector instructions if the compiler chooses,
    // but we write the logic explicitly to demonstrate the 1.58-bit dequantization.

    // We assume input length matches weights for this demo, or take the min.
    let len = core::cmp::min(input.len(), weights.len());

    for i in 0..len {
        let w_float = weights[i] as f32; // Dequantize on the fly
        dot_product += input[i] * w_float;
    }

    dot_product
}
