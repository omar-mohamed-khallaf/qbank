use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, info, warn};

use crate::error::AppError;

const TUTOR_PROMPT: &str = r#"
Role: Expert Medical Board Tutor. You excel at USMLE/COMLEX-style clinical reasoning, identifying "distractor" patterns, and synthesizing complex patient data into concise differentials.

Task: Analyze the provided medical cases QUICKLY and return a raw JSON array of objects. Each object represents one question.

JSON Schema & Requirements:
- "q": Copy the question stem exactly as provided (do not include options).
- "c": An array of objects:
    {"text": "Option string", "r": boolean}
    - Preserve option text EXACTLY as given.
    - Set "r": true for ONLY the single best answer.
- "e": an object containing:
    1. "clues": Array of strings of key diagnostic pivot points (age, symptoms, labs, etc.).
    2. "rep": String of one-sentence formal problem representation using medical terminology.
    3. "logic": String of step-by-step reasoning from presentation to correct answer.
    4. "diff": Array of strings of rule-out reasoning for EACH incorrect option.
    5. "pearls": Array of strings of 3–5 high-yield board facts.

Strict Rules:
- Do NOT include option labels (A, B, etc.).
- Do NOT introduce new options.
- Exactly ONE option must have "r": true.
- All incorrect options appear in "diff".

Formatting Rules:
- Output ONLY a raw JSON array.
- Start with [ and end with ].
- No markdown, no extra text.
- Ensure valid JSON (parsable).
- Escape all internal double quotes (\").
- Use \n for line breaks inside strings.

Medical Accuracy:
- If ambiguity exists, choose the most likely "Gold Standard" or "Next Best Step" based on current clinical guidelines.
- If a question is outdated, choose the most commonly expected board-style answer, not necessarily modern practice.
- Prefer widely accepted board answers over edge-case interpretations.

Self-Check (before output):
- JSON is valid and complete
- Exactly one correct answer
- All options preserved exactly
- All incorrect options addressed in "diff"

Return only the JSON array.
"#;

const NORMALIZER_PROMPT: &str = r#"
Role: Fast Medical Question Normalizer

You are a high-speed data-cleaning engine. Convert messy, unstructured multiple-choice medical questions into clean, structured JSON immediately.

Task:
1. Extract the question stem.
2. Extract and reconstruct answer choices.
3. Remove all labels or prefixes (a, b, c, d, A., etc.).
4. Split merged or concatenated options.
5. Correct obvious medical word errors (e.g., "ethyldopa" → "methyldopa", "ARBs" → "Angiotensin receptor blockers").
6. Remove meaningless fragments or stray tokens.
7. Ensure each option is medically valid, readable, and standalone.

Rules:
- Output 3–6 answer choices.
- No labels or prefixes.
- No duplicate or overlapping options.
- Reconstruct the most likely valid medical options if input is corrupted.
- Discard unclear fragments that cannot be confidently fixed.
- Do NOT explain or comment.
- Ensure each option is complete and meaningful

Output format (STRICT JSON ONLY):
{
"q": "<clean question stem>",
"options": ["option 1", "option 2", "option 3", "option 4"]
}

Constraints:
- Return ONLY valid JSON.
- No markdown, no comments, no trailing text.
- Use double quotes only.
- Ensure JSON parses correctly.

Input:
"#;

const JSON_REPAIR_PROMPT: &str = r#"
Role: JSON Repair Engine

You are a strict JSON fixer. Your ONLY job is to repair invalid or malformed JSON so that it becomes valid and parsable.

Input:
You will receive a JSON-like string that may contain errors such as:
- Missing commas
- Trailing commas
- Unescaped quotes
- Invalid characters
- Broken structure
- Incorrect brackets

Task:
- Fix the JSON so it is syntactically valid.
- Preserve ALL original data and structure.
- Do NOT change meanings, wording, or values.
- Do NOT remove fields unless absolutely necessary for validity.
- Do NOT add new content.

Rules:
- Output must be valid JSON.
- Output must match the original schema as closely as possible.
- Ensure all strings use double quotes.
- Escape internal quotes properly (\").
- Ensure arrays and objects are properly closed.
- Remove trailing commas.
- Fix broken nesting.

Strict Output Rules:
- Return ONLY the fixed JSON.
- No explanations.
- No markdown.
- No comments.
- Must start with { or [ and end with } or ].

Failure Handling:
- If the input is too corrupted to fully repair, return the closest valid JSON possible while preserving maximum content.

Self-Check:
- JSON parses without error
- No trailing commas
- Proper escaping
- Structure intact

Now fix the following JSON:
"#;

#[derive(Debug, Deserialize)]
struct NormalizedQuestion {
    q: String,
    options: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    think: bool,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    message: OllamaMessage,
    done: bool,
    done_reason: Option<String>,
    created_at: Option<String>,
    total_duration: Option<u64>,
    #[serde(rename = "prompt_eval_count")]
    prompt_eval_count: Option<i32>,
    #[serde(rename = "eval_count")]
    eval_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    thinking: Option<String>,
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
    pub q: String,
    pub c: Vec<ChoiceResponse>,
    pub e: ExplanationResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceResponse {
    pub text: String,
    pub r: bool,
}

#[derive(Clone)]
pub struct LlmClient {
    http_client: Client,
    model: String,
    max_retries: i32,
    retry_delay_ms: u64,
    retry_multiplier: f64,
    think: bool,
    stream: bool,
}

impl LlmClient {
    pub fn new(
        model: String,
        max_retries: i32,
        retry_delay_ms: u64,
        retry_multiplier: f64,
        think: bool,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            http_client,
            model,
            max_retries,
            retry_delay_ms,
            retry_multiplier,
            think,
            stream: false,
        }
    }

    pub async fn process_medical_question(
        &self,
        raw_text: &str,
    ) -> Result<Vec<QuestionResponse>, AppError> {
        let normalized = self.normalize_question_input(raw_text).await?;

        let options_formatted = normalized
            .options
            .iter()
            .map(|o| format!("- {o}"))
            .collect::<Vec<_>>()
            .join("\n");

        let tutor_prompt = format!(
            "{TUTOR_PROMPT}\n\nQUESTION:\n{}\n\nOPTIONS:\n{}",
            normalized.q, options_formatted
        );

        self.send_tutor_request_with_retry(&tutor_prompt).await
    }

    async fn normalize_question_input(
        &self,
        raw_text: &str,
    ) -> Result<NormalizedQuestion, AppError> {
        let mut attempt = 0;
        let mut delay_ms = self.retry_delay_ms;

        loop {
            attempt += 1;
            info!("Normalization attempt {}/{}", attempt, self.max_retries + 1);

            let prompt = format!("{NORMALIZER_PROMPT}\n{raw_text}");

            let request = ChatRequest {
                model: self.model.clone(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                }],
                think: false,
                stream: false,
            };

            match self
                .http_client
                .post("http://localhost:11434/api/chat")
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    let bytes = response.bytes().await.unwrap_or_default();
                    let ollama_response: OllamaResponse = serde_json::from_slice(&bytes)?;
                    let content = ollama_response.message.content;

                    let parsed: NormalizedQuestion = if let Ok(v) = serde_json::from_str(&content) {
                        v
                    } else {
                        let fixed = self.repair_json_output(&content).await?;
                        serde_json::from_str(&fixed).map_err(|e| {
                            AppError::Api(format!(
                                "Failed to parse repaired normalization output: {e} - Content: {fixed}"
                            ))
                        })?
                    };

                    info!("Normalization successful on attempt {}", attempt);
                    return Ok(parsed);
                }
                Ok(response) => {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    let err = AppError::Api(format!(
                        "Normalization API returned status {status}: {error_text}"
                    ));
                    if attempt <= self.max_retries {
                        warn!("Normalization failed (attempt {}): {}", attempt, err);
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        delay_ms = (delay_ms as f64 * self.retry_multiplier) as u64;
                    } else {
                        return Err(err);
                    }
                }
                Err(e) => {
                    let err = AppError::Api(format!("Normalization request failed: {e}"));
                    if attempt <= self.max_retries {
                        warn!("Normalization failed (attempt {}): {}", attempt, e);
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        delay_ms = (delay_ms as f64 * self.retry_multiplier) as u64;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }

    async fn repair_json_output(&self, broken_json: &str) -> Result<String, AppError> {
        let prompt = format!("{JSON_REPAIR_PROMPT}\n{broken_json}");

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            think: false,
            stream: false,
        };

        let response = self
            .http_client
            .post("http://localhost:11434/api/chat")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "JSON repair API returned status {status}: {error_text}"
            )));
        }

        let bytes = response.bytes().await.unwrap_or_default();
        let ollama_response: OllamaResponse = serde_json::from_slice(&bytes)?;
        let content = ollama_response.message.content;

        Ok(content)
    }

    async fn send_tutor_request_with_retry(
        &self,
        prompt: &str,
    ) -> Result<Vec<QuestionResponse>, AppError> {
        let mut attempt = 0;
        let mut delay_ms = self.retry_delay_ms;
        let base_prompt = prompt.to_string();

        loop {
            attempt += 1;
            info!("Tutor request attempt {}/{}", attempt, self.max_retries + 1);

            let enhanced_prompt = match attempt {
                1 => base_prompt.clone(),
                2 => format!("{base_prompt}\n\nFix formatting. Return valid JSON only."),
                _ => format!(
                    "{base_prompt}\n\nYour previous output was invalid. Output MUST be valid JSON with exactly one correct answer."
                ),
            };

            match self.execute_tutor_request(&enhanced_prompt).await {
                Ok(response) => {
                    info!("Tutor request successful on attempt {}", attempt);
                    return Ok(response);
                }
                Err(e) if attempt <= self.max_retries => {
                    warn!("Tutor request failed (attempt {}): {}", attempt, e);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        clippy::cast_precision_loss
                    )]
                    {
                        let next_delay_f64 = delay_ms as f64 * self.retry_multiplier;
                        delay_ms = next_delay_f64.clamp(0.0, u64::MAX as f64) as u64;
                    }
                }
                Err(e) => {
                    error!("Tutor request failed after {} attempts: {}", attempt, e);
                    return Err(e);
                }
            }
        }
    }

    async fn execute_tutor_request(&self, prompt: &str) -> Result<Vec<QuestionResponse>, AppError> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            think: self.think,
            stream: self.stream,
        };

        let response = self
            .http_client
            .post("http://localhost:11434/api/chat")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::Api(format!(
                "API returned status {status}: {error_text}"
            )));
        }

        let bytes = response.bytes().await.unwrap_or_default();
        let ollama_response: OllamaResponse = serde_json::from_slice(&bytes)?;

        let content = ollama_response.message.content;

        let parsed: Vec<QuestionResponse> = if let Ok(v) = serde_json::from_str(&content) {
            v
        } else {
            let fixed = self.repair_json_output(&content).await?;
            serde_json::from_str(&fixed).map_err(|e| {
                AppError::Api(format!(
                    "Failed to parse repaired tutor output: {e} - Content: {fixed}"
                ))
            })?
        };

        let correct_count = parsed.iter().flat_map(|q| &q.c).filter(|c| c.r).count();

        if correct_count != 1 {
            return Err(AppError::Api(format!(
                "Invalid correct answer count: expected 1, got {correct_count}"
            )));
        }

        Ok(parsed)
    }
}
