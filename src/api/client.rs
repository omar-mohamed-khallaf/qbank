use rand::random;
use std::num::NonZeroU32;
use std::path::Path;

use encoding_rs::UTF_8;
use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::{LlamaBackendDevice, list_llama_ggml_backend_devices};
use serde::{Deserialize, Serialize};

use crate::api::prompts::{NORMALIZER_PROMPT, TUTOR_PROMPT};
use crate::error::AppError;

#[derive(Debug, Deserialize)]
struct NormalizedQuestion {
    question: String,
    choices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationResponse {
    pub clues: Vec<String>,
    pub rep: String,
    pub logic: String,
    pub diff: Vec<String>,
    pub pearls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionResponse {
    pub question: String,
    pub choices: Vec<ChoiceResponse>,
    pub explanation: ExplanationResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceResponse {
    pub text: String,
    pub correct: bool,
}

pub struct LlmClient {
    model: LlamaModel,
    backend: LlamaBackend,
    sampler: LlamaSampler,
    context_size: u32,
    think: bool,
}

impl LlmClient {
    pub fn new(
        model_path: &Path,
        context_size: u32,
        devices: Option<Vec<usize>>,
        think: bool,
    ) -> Result<Self, AppError> {
        let backend = LlamaBackend::init().map_err(|e| AppError::Api(e.to_string()))?;
        unsafe {
            llama_cpp_sys_2::llama_log_set(Some(Self::void_log), std::ptr::null_mut());
        }

        let mut model_params = LlamaModelParams::default();
        model_params = model_params.with_n_gpu_layers(16384);

        if let Some(device_indices) = devices {
            model_params = model_params
                .with_devices(&device_indices)
                .map_err(|e| AppError::Api(e.to_string()))?;
        }

        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| AppError::Api(format!("Failed to load model: {e}")))?;

        let sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(0.6),
            LlamaSampler::top_p(0.95, 1),
            LlamaSampler::min_p(0.05, 1),
            LlamaSampler::dist(random::<u32>()),
        ]);

        Ok(Self {
            model,
            backend,
            sampler,
            context_size,
            think,
        })
    }

    unsafe extern "C" fn void_log(
        level: llama_cpp_sys_2::ggml_log_level,
        text: *const ::std::os::raw::c_char,
        _user_data: *mut ::std::os::raw::c_void,
    ) {
        let _ = std::panic::catch_unwind(|| {
            static COUNTER: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
            let prev = *COUNTER.get_or_init(|| level);

            if text.is_null() {
                return;
            }

            let level = if level == 5 { prev } else { level };

            let c_str = unsafe { std::ffi::CStr::from_ptr(text) };

            let log_text = c_str.to_string_lossy();

            match level {
                0 => tracing::trace!("{}", log_text.trim()),
                1 => tracing::debug!("{}", log_text.trim()),
                2 => tracing::info!("{}", log_text.trim()),
                3 => tracing::warn!("{}", log_text.trim()),
                4 => tracing::error!("{}", log_text.trim()),
                _ => tracing::trace!("{}", log_text.trim()),
            }
        });
    }

    pub fn process_medical_question(
        &mut self,
        raw_text: &str,
    ) -> Result<QuestionResponse, AppError> {
        let normalized = self.normalize_question_input(raw_text)?;

        let options_formatted = normalized.choices.join("\n");

        let tutor_prompt = format!(
            "{TUTOR_PROMPT}\nQuestion:{}\nOptions:{}\n",
            normalized.question, options_formatted
        );

        let mut last_error = None;
        for i in 0..2 {
            let content = self.generate_sync(&tutor_prompt, true)?;

            match serde_json::from_str::<QuestionResponse>(&content) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    tracing::warn!("Tutor parse attempt {} failed: {e}", i + 1);
                    last_error = Some(e);
                }
            }
        }

        let err = last_error.map_or_else(|| "Unknown error".into(), |e| e.to_string());
        Err(AppError::Api(format!(
            "Tutor failed to generate valid JSON: {err}"
        )))
    }

    fn normalize_question_input(&mut self, raw_text: &str) -> Result<NormalizedQuestion, AppError> {
        let prompt = format!("{NORMALIZER_PROMPT}\n{raw_text}");
        let max_attempts = 3;

        for i in 0..max_attempts {
            let content = self.generate_sync(&prompt, true)?;

            match serde_json::from_str::<NormalizedQuestion>(&content) {
                Ok(normalized) => return Ok(normalized),
                Err(e) => {
                    if i == max_attempts - 1 {
                        tracing::error!(
                            "Final attempt failed. Parse error: {e}. Content: {content}"
                        );
                        return Err(AppError::Api(format!(
                            "JSON parse error after {max_attempts} tries"
                        )));
                    }

                    tracing::warn!(
                        "Attempt {}/{} failed to parse JSON. Retrying...",
                        i + 1,
                        max_attempts
                    );
                }
            }
        }

        Err(AppError::Api("Unknown normalization error".into()))
    }

    fn generate_sync(&mut self, prompt: &str, think: bool) -> Result<String, AppError> {
        let full_prompt = format!(
            "<｜User｜>{prompt}<｜Assistant｜>{}",
            if think { "" } else { "<think>\n\n\n" }
        );

        let tokens = self
            .model
            .str_to_token(&full_prompt, AddBos::Always)
            .map_err(|e| AppError::Api(e.to_string()))?;

        let n_tokens = tokens.len() as u32;

        if n_tokens >= self.context_size {
            return Err(AppError::Api("Prompt too long for context size".into()));
        }
        let max_tokens = self.context_size - n_tokens;

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(self.context_size))
            .with_flash_attention_policy(1)
            .with_n_threads(8)
            .with_type_v(KvCacheType::Q4_0)
            .with_type_k(KvCacheType::Q4_0);

        let mut context = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| AppError::Api(e.to_string()))?;

        let mut batch = LlamaBatch::new(n_tokens.max(512) as usize, 1);

        batch
            .add_sequence(&tokens, 0, true)
            .map_err(|e| AppError::Api(e.to_string()))?;
        context
            .decode(&mut batch)
            .map_err(|e| AppError::Api(e.to_string()))?;

        let mut generated_tokens = Vec::new();

        for i in 0..max_tokens {
            let token = self.sampler.sample(&context, 0);

            if token == self.model.token_eos() {
                break;
            }

            generated_tokens.push(token);

            batch.clear();
            batch
                .add(token, (n_tokens + i) as i32, &[0], true)
                .map_err(|e| AppError::Api(e.to_string()))?;

            context
                .decode(&mut batch)
                .map_err(|e| AppError::Api(e.to_string()))?;
        }

        let mut decoder = UTF_8.new_decoder();
        let mut output = String::new();
        for t in &generated_tokens {
            let piece = self
                .model
                .token_to_piece(*t, &mut decoder, true, None)
                .map_err(|e| AppError::Api(e.to_string()))?;
            output.push_str(&piece);
        }

        if output.trim().is_empty() {
            return Err(AppError::Api("Generated output is empty".to_string()));
        }

        let json_str = {
            let body = if let Some(think_end) = output.find("</think>") {
                &output[think_end + 8..]
            } else {
                &output
            };

            if let Some(start_idx) = body.find('{')
                && let Some(end_idx) = body.rfind('}')
                && end_idx > start_idx
            {
                &body[start_idx..=end_idx]
            } else {
                body.trim()
            }
        };

        Ok(json_str.to_string())
    }
}

pub fn list_devices() -> Vec<LlamaBackendDevice> {
    list_llama_ggml_backend_devices()
}
