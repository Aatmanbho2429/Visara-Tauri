use crate::{
    config::{model_enc_path, BATCH_SIZE, CLIP_INPUT_SIZE, CLIP_MEAN, CLIP_STD, EMB_DIM},
    error::{Result, VisaraError},
};
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
    log::info!("Loading CLIP model from {:?}", model_path);

    if !model_path.exists() {
        return Err(VisaraError::Fatal(format!(
            "Model file not found: {:?}", model_path
        )));
    }

    let encrypted   = std::fs::read(&model_path)?;
    let token_str   = String::from_utf8_lossy(&encrypted);
    let model_bytes = fernet_decrypt(key, token_str.trim())?;

    let session = Session::builder()
        .map_err(|e| VisaraError::Fatal(e.to_string()))?
        .with_intra_threads(4)
        .map_err(|e| VisaraError::Fatal(e.to_string()))?
        .with_inter_threads(4)
        .map_err(|e| VisaraError::Fatal(e.to_string()))?
        .commit_from_memory(&model_bytes)
        .map_err(|e| VisaraError::Fatal(e.to_string()))?;

    let input_name = session.inputs()[0].name().to_string();
    let out_name   = session.outputs()[0].name().to_string();

    log::info!("CLIP model ready  input={input_name}  output={out_name}");

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
    let state = guard.as_mut().ok_or(VisaraError::ModelNotReady)?;

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
            .map_err(|e| VisaraError::Fatal(e.to_string()))?;

        // inputs! in ort rc.12 returns Vec directly — no ? needed.
        let outputs = state.session.run(
            inputs![state.input_name.as_str() => input_value]
        )?;

        // try_extract_tensor returns (&Shape, &[T]) in ort rc.12.
        let (_shape, out_data) = outputs[state.out_name.as_str()]
            .try_extract_tensor::<f32>()
            .map_err(|e| VisaraError::Fatal(e.to_string()))?;

        for i in 0..chunk_n {
            let row = out_data[i * EMB_DIM..(i + 1) * EMB_DIM].to_vec();
            let norm = row.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
            all_embeddings.push(row.into_iter().map(|v| v / norm).collect());
        }
    }

    Ok(all_embeddings)
}

pub fn preprocess(img: image::DynamicImage) -> Vec<f32> {
    let size = CLIP_INPUT_SIZE as usize;
    let resized = img
        .resize_exact(CLIP_INPUT_SIZE, CLIP_INPUT_SIZE, image::imageops::FilterType::Triangle)
        .to_rgb8();

    let mut output = vec![0.0_f32; 3 * size * size];
    for (i, pixel) in resized.pixels().enumerate() {
        let row = i / size;
        let col = i % size;
        for c in 0..3 {
            let val        = pixel[c] as f32 / 255.0;
            let normalised = (val - CLIP_MEAN[c]) / CLIP_STD[c];
            output[c * size * size + row * size + col] = normalised;
        }
    }
    output
}

pub const EMBED_BATCH_SIZE: usize = BATCH_SIZE;

// Pure-Rust Fernet decryption (no OpenSSL).
// Spec: https://github.com/fernet/spec/blob/master/Spec.md
// Token  = URL_SAFE_BASE64( 0x80 || Time(8BE) || IV(16) || Ciphertext || HMAC(32) )
// Key    = URL_SAFE_BASE64( signing_key(16) || encryption_key(16) )
fn fernet_decrypt(key_b64: &str, token_b64: &str) -> Result<Vec<u8>> {
    let key_bytes = URL_SAFE.decode(key_b64).map_err(|_| VisaraError::Decryption)?;
    if key_bytes.len() != 32 {
        return Err(VisaraError::Decryption);
    }
    let signing_key    = &key_bytes[..16];
    let encryption_key = &key_bytes[16..];

    let token = URL_SAFE.decode(token_b64).map_err(|_| VisaraError::Decryption)?;
    if token.len() < 73 || token[0] != 0x80 {
        return Err(VisaraError::Decryption);
    }

    let payload    = &token[..token.len() - 32];
    let token_hmac = &token[token.len() - 32..];
    let iv         = &token[9..25];
    let ciphertext = &token[25..token.len() - 32];

    let mut mac = HmacSha256::new_from_slice(signing_key).map_err(|_| VisaraError::Decryption)?;
    mac.update(payload);
    mac.verify_slice(token_hmac).map_err(|_| VisaraError::Decryption)?;

    let mut buf = ciphertext.to_vec();
    let plaintext = Aes128CbcDec::new(encryption_key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| VisaraError::Decryption)?;

    Ok(plaintext.to_vec())
}
