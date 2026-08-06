use crate::{
    config::{model_enc_path, BATCH_SIZE, CLIP_INPUT_SIZE, CLIP_MEAN, CLIP_STD, EMB_DIM},
    error::{Result, PictoriaError},
};
#[cfg(target_os = "macos")]
use ort::execution_providers::CoreMLExecutionProvider;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use hmac::{Hmac, Mac};
use once_cell::sync::Lazy;
use ort::{inputs, session::Session, value::Value};
use sha2::Sha256;
use std::sync::Mutex;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type HmacSha256   = Hmac<Sha256>;

struct EmbedderState {
    session:    Session,
    input_name: String,
    out_name:   String,
}

// Mutex (not RwLock) because Session::run() in ort rc.12 requires &mut self.
static STATE: Lazy<Mutex<Option<EmbedderState>>> =
    Lazy::new(|| Mutex::new(None));

pub fn is_ready() -> bool {
    STATE.lock().unwrap().is_some()
}

pub fn load_model(key: &str) -> Result<()> {
    // Early-out without holding the lock during the expensive decrypt + compile.
    if STATE.lock().unwrap().is_some() {
        return Ok(());
    }

    let model_path = model_enc_path();
    log::info!("Loading DINOv2 model from {:?}", model_path);

    if !model_path.exists() {
        return Err(PictoriaError::Fatal(format!(
            "Model file not found: {:?}", model_path
        )));
    }

    let encrypted   = std::fs::read(&model_path)?;
    let token_str   = String::from_utf8_lossy(&encrypted);
    let model_bytes = fernet_decrypt(key, token_str.trim())?;

    let builder = Session::builder()
        .map_err(|e| PictoriaError::Fatal(e.to_string()))?
        .with_intra_threads(4)
        .map_err(|e| PictoriaError::Fatal(e.to_string()))?
        .with_inter_threads(4)
        .map_err(|e| PictoriaError::Fatal(e.to_string()))?;

    // On macOS, try CoreML first (Neural Engine on Apple Silicon, Metal on Intel).
    // If the hardware/OS doesn't support it, ort falls back to CPU automatically.
    #[cfg(target_os = "macos")]
    let builder = builder
        .with_execution_providers([CoreMLExecutionProvider::default().build()])
        .map_err(|e| PictoriaError::Fatal(e.to_string()))?;

    let session = builder
        .commit_from_memory(&model_bytes)
        .map_err(|e| PictoriaError::Fatal(e.to_string()))?;

    let input_name = session.inputs[0].name.clone();
    let out_name   = session.outputs[0].name.clone();

    log::info!("DINOv2 model ready  input={input_name}  output={out_name}");

    *STATE.lock().unwrap() = Some(EmbedderState { session, input_name, out_name });
    Ok(())
}

pub fn reset() {
    *STATE.lock().unwrap() = None;
    log::info!("Embedder reset");
}

pub fn embed_batch(images: &[f32], batch_size: usize) -> Result<Vec<Vec<f32>>> {
    let pixels_per_img = 3 * CLIP_INPUT_SIZE as usize * CLIP_INPUT_SIZE as usize;
    let n = images.len() / pixels_per_img;
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut all_embeddings: Vec<Vec<f32>> = Vec::with_capacity(n);

    // Lock once for the entire batch — Session::run() needs &mut self.
    let mut guard = STATE.lock().unwrap();
    let state = guard.as_mut().ok_or(PictoriaError::ModelNotReady)?;

    for chunk_start in (0..n).step_by(batch_size) {
        let chunk_end  = (chunk_start + batch_size).min(n);
        let chunk_n    = chunk_end - chunk_start;
        let chunk_data = images[chunk_start * pixels_per_img..chunk_end * pixels_per_img].to_vec();

        let shape: Vec<i64> = vec![
            chunk_n as i64,
            3,
            CLIP_INPUT_SIZE as i64,
            CLIP_INPUT_SIZE as i64,
        ];

        let input_value = Value::from_array((shape, chunk_data))
            .map_err(|e| PictoriaError::Fatal(e.to_string()))?;

        // inputs! in ort rc.12 returns Vec directly — no ? needed.
        let outputs = state.session.run(
            inputs![state.input_name.as_str() => input_value]
        )?;

        // try_extract_tensor returns (&Shape, &[T]) in ort rc.12.
        let (_shape, out_data) = outputs[state.out_name.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|e| PictoriaError::Fatal(e.to_string()))?;

        for i in 0..chunk_n {
            let row = out_data[i * EMB_DIM..(i + 1) * EMB_DIM].to_vec();
            let norm = row.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
            all_embeddings.push(row.into_iter().map(|v| v / norm).collect());
        }
    }

    Ok(all_embeddings)
}

/// Preprocess for pattern/design embedding: convert to grayscale first so the
/// resulting CLIP vector encodes structure and texture only, not hue.  When
/// replicated across all three channels, luma passes through CLIP's normalization
/// unchanged — the model sees a neutral-grey version of every tile, making "same
/// pattern, different color" produce nearly identical design vectors.
pub fn preprocess_grayscale(img: image::DynamicImage) -> Vec<f32> {
    preprocess(image::DynamicImage::ImageLuma8(img.into_luma8()))
}

/// Canonical recipe: resize so the *shorter* side == 224 (aspect preserved),
/// then centre-crop to 224×224.
///
/// ## Why not pad to square instead
/// Padding looks like the obvious way to avoid discarding the outer parts of a
/// non-square image, and it was tried.  It is actively harmful: a 1:2 tile
/// becomes a 112-wide strip with mirrored fill on either side, and *every* 1:2
/// image acquires the same band structure at the same positions.  That shared
/// artefact then dominates the embedding — measured on a real library, all 20
/// top hits for a 1.98-aspect query were 1.98-aspect images regardless of their
/// design, while the true parent image (a 1.50-aspect landscape carrying the
/// exact same marble) fell to rank 237.  Aspect ratio became the search.
///
/// It also silently broke partial matching: region slices are square and so
/// carry no padding, which put them in a different visual domain from a padded
/// query and left them unable to compete.  Removing the padding moved that same
/// query's true parent from rank 237 to rank 2.
///
/// Nothing is lost by cropping here, because full-frame coverage is the job of
/// [`crate::core::regions`] — its windows span the whole image, edges included.
/// The centre crop is just the coarsest view among them.
pub fn preprocess(img: image::DynamicImage) -> Vec<f32> {
    let size = CLIP_INPUT_SIZE;   // 224
    let sz   = size as usize;

    let (w, h) = (img.width(), img.height());
    let (nw, nh) = if w <= h {
        (size, ((h as u64 * size as u64) / w.max(1) as u64) as u32)
    } else {
        (((w as u64 * size as u64) / h.max(1) as u64) as u32, size)
    };
    let resized = img.resize_exact(
        nw.max(size),
        nh.max(size),
        image::imageops::FilterType::Triangle,
    );
    let x = resized.width().saturating_sub(size) / 2;
    let y = resized.height().saturating_sub(size) / 2;
    let cropped = resized.crop_imm(x, y, size, size).to_rgb8();

    let mut output = vec![0.0_f32; 3 * sz * sz];
    for (i, pixel) in cropped.pixels().enumerate() {
        let row = i / sz;
        let col = i % sz;
        for c in 0..3 {
            let val        = pixel[c] as f32 / 255.0;
            let normalised = (val - CLIP_MEAN[c]) / CLIP_STD[c];
            output[c * sz * sz + row * sz + col] = normalised;
        }
    }
    output
}

pub const EMBED_BATCH_SIZE: usize = BATCH_SIZE;

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage};

    const SZ: usize = CLIP_INPUT_SIZE as usize;

    /// Tall image whose left third is white and the rest black — asymmetric
    /// across the vertical axis, which is what makes padding detectable.
    fn left_banded_tall() -> DynamicImage {
        let (w, h) = (200u32, 400u32);
        let mut img = RgbImage::from_pixel(w, h, Rgb([0, 0, 0]));
        for y in 0..h {
            for x in 0..(w / 3) {
                img.put_pixel(x, y, Rgb([255, 255, 255]));
            }
        }
        DynamicImage::ImageRgb8(img)
    }

    /// Regression test for the mirror-padding that made aspect ratio the
    /// dominant search signal (true parent image fell to rank 237; removing the
    /// padding put it at rank 2).
    ///
    /// Under padding, a 1:2 image was resized to a 112-wide strip centred in the
    /// frame, so column 0 held *mirrored* content — here, black.  Under the
    /// centre crop the resized image spans the full width, so column 0 holds the
    /// real left edge — white.  That single column tells the two apart.
    #[test]
    fn preprocessing_does_not_pad_non_square_images() {
        let t = preprocess(left_banded_tall());
        assert_eq!(t.len(), 3 * SZ * SZ);

        let mid_row = SZ / 2;
        let left_edge  = t[mid_row * SZ];
        let right_edge = t[mid_row * SZ + SZ - 1];

        assert!(
            left_edge > 0.0,
            "column 0 should be the image's real left edge (white), got {left_edge} — \
             a negative value means the frame was padded again"
        );
        assert!(
            right_edge < 0.0,
            "column 223 should be the image's real right edge (black), got {right_edge}"
        );
    }

    #[test]
    fn square_images_pass_through_undistorted() {
        // No padding is needed for a square, so this is the plain resize path —
        // the common case, and the one that must not regress.
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(300, 300, Rgb([128, 64, 32])));
        let t = preprocess(img);
        assert_eq!(t.len(), 3 * SZ * SZ);

        let expected = (128.0 / 255.0 - CLIP_MEAN[0]) / CLIP_STD[0];
        for &v in t.iter().take(SZ * SZ) {
            assert!((v - expected).abs() < 0.05, "flat square should embed flat");
        }
    }
}

// Pure-Rust Fernet decryption (no OpenSSL).
// Spec: https://github.com/fernet/spec/blob/master/Spec.md
// Token  = URL_SAFE_BASE64( 0x80 || Time(8BE) || IV(16) || Ciphertext || HMAC(32) )
// Key    = URL_SAFE_BASE64( signing_key(16) || encryption_key(16) )
fn fernet_decrypt(key_b64: &str, token_b64: &str) -> Result<Vec<u8>> {
    let key_bytes = URL_SAFE.decode(key_b64).map_err(|_| PictoriaError::Decryption)?;
    if key_bytes.len() != 32 {
        return Err(PictoriaError::Decryption);
    }
    let signing_key    = &key_bytes[..16];
    let encryption_key = &key_bytes[16..];

    let token = URL_SAFE.decode(token_b64).map_err(|_| PictoriaError::Decryption)?;
    if token.len() < 73 || token[0] != 0x80 {
        return Err(PictoriaError::Decryption);
    }

    let payload    = &token[..token.len() - 32];
    let token_hmac = &token[token.len() - 32..];
    let iv         = &token[9..25];
    let ciphertext = &token[25..token.len() - 32];

    let mut mac = HmacSha256::new_from_slice(signing_key).map_err(|_| PictoriaError::Decryption)?;
    mac.update(payload);
    mac.verify_slice(token_hmac).map_err(|_| PictoriaError::Decryption)?;

    let mut buf = ciphertext.to_vec();
    let plaintext = Aes128CbcDec::new(encryption_key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| PictoriaError::Decryption)?;

    Ok(plaintext.to_vec())
}
